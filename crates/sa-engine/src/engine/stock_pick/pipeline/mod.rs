use std::collections::{HashMap, HashSet};

use anyhow::Context;
use futures::{StreamExt, stream};

use crate::data::{BillboardEntry, CapitalFlowPoint, MarketDataClient, MarketKind, NewsItem};
use crate::engine::llm::{self as llm, LlmClient};
use crate::models::{
    StockPickFactorBreakdown, StockPickHistoryMatchSnapshot, StockPickItem,
    StockPickObjectiveAssessment, StockPickRequest, StockPickResponse,
    StockPickSelectionDiagnostics, StockPickStorageWriteSummary,
};

use crate::engine::stock_pick::{
    CandidateContext, CandidateEvidenceRecord, EnrichedCandidate,
    StockPickEvidencePayload, StockPickHistoryStore, parse_generated_stock_pick,
};

use crate::engine::stock_pick::{
    apply_portfolio_constraints, enrich_candidates, infer_theme_key, score_candidates,
};

use crate::engine::stock_pick::objective::{
    build_prompt, default_catalysts, default_evidence, default_risks, default_thesis,
    evaluate_stock_pick_objective_assessment, stock_pick_priority_label, stock_pick_priority_rank,
    stock_pick_sort_key, summarize_stock_pick_objective_overview,
};

pub async fn run(
    market_data: &MarketDataClient,
    llm_client: &LlmClient,
    request: &StockPickRequest,
) -> anyhow::Result<StockPickResponse> {
    let analysis_date = request
        .analysis_date
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let strategy = request
        .strategy
        .clone()
        .unwrap_or_else(|| "balanced swing selection".to_string());
    let language = "zh-CN".to_string();
    let search_depth = normalize_stock_pick_search_depth(request.search_depth.as_deref());
    let history_retrieval = request.history_retrieval.unwrap_or(true);
    let target_output_mode = normalize_target_output_mode(request.target_output_mode.as_deref());
    let candidate_limit = request.candidate_limit.unwrap_or(12).clamp(6, 30);
    let pick_count = request.pick_count.unwrap_or(3).clamp(1, 3);
    let coarse_candidate_limit = derive_coarse_candidate_limit(candidate_limit, search_depth);
    let deep_candidate_limit = derive_deep_candidate_limit(pick_count, search_depth);
    let llm_review_limit = derive_llm_review_limit(pick_count, search_depth);
    let history_store = StockPickHistoryStore::from_env()
        .context("stock pick history store initialization failed")?;

    // Fetch daily guidance for context enrichment
    let guidance_context = match crate::engine::guidance::GuidanceStore::from_env()
        .get_latest_stock_pick_summary(&request.market)
        .await
    {
        Ok(Some(summary)) => {
            let sentiment = summary
                .get("market_sentiment")
                .and_then(|v| v.get("label"))
                .and_then(|v| v.as_str())
                .unwrap_or("neutral");
            format!(
                "Market sentiment: {}. Recent picks: {}",
                sentiment,
                summary
                    .get("picks")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr
                        .iter()
                        .filter_map(|p| p.get("symbol").and_then(|v| v.as_str()))
                        .collect::<Vec<_>>()
                        .join(", "))
                    .unwrap_or_default()
            )
        }
        _ => String::new(),
    };

    let candidates =
        resolve_candidates(market_data, request, coarse_candidate_limit).await?;
    if candidates.is_empty() {
        anyhow::bail!("no stock candidates resolved for market {}", request.market);
    }

    let light_queries = if should_skip_light_stage_search(request, &candidates) {
        tracing::info!(
            market = %request.market,
            candidate_count = candidates.len(),
            "stock pick light-stage search skipped for explicit candidate set"
        );
        Vec::new()
    } else {
        build_light_search_queries(request, &candidates)
    };
    let search_time_range = stock_pick_search_time_range(search_depth);
    tracing::info!(
        market = %request.market,
        candidate_count = candidates.len(),
        queries = ?light_queries,
        time_range = ?search_time_range,
        "stock pick light-stage queries"
    );
    let _light_evidence: Vec<crate::data::NewsItem> = Vec::new();

    let mut enriched = enrich_candidates(market_data, &candidates, deep_candidate_limit).await;
    score_candidates(&mut enriched);

    let is_explicit_set = request
        .candidate_symbols
        .as_ref()
        .is_some_and(|symbols| !symbols.is_empty());
    let filtered = if is_explicit_set {
        // For explicit candidates, keep all enriched items regardless of pass_filter
        enriched.clone()
    } else {
        enriched
            .iter()
            .filter(|item| item.pass_filter)
            .cloned()
            .collect::<Vec<_>>()
    };
    if filtered.is_empty() {
        anyhow::bail!("all candidates were filtered out before stock selection");
    }

    let mut deep_pool = apply_portfolio_constraints(filtered, deep_candidate_limit);
    if deep_pool.is_empty() {
        anyhow::bail!("deep candidate pool is empty after portfolio constraints");
    }

    let mut indexed_evidence_records = 0usize;
    for candidate in deep_pool.iter_mut() {
        let deep_queries = build_candidate_search_queries(candidate, request);
        let search_items: Vec<crate::data::NewsItem> = Vec::new();
        if !search_items.is_empty() {
            let deduped_records = news_items_to_evidence_records(
                &candidate.symbol,
                &candidate.market,
                &candidate.theme_key,
                &deep_queries,
                &search_items,
            );
            indexed_evidence_records += deduped_records.len();
            candidate.news = crate::data::news::dedupe_news_items(
                candidate
                    .news
                    .iter()
                    .cloned()
                    .chain(search_items)
                    .collect(),
            );
            candidate.evidence_records = deduped_records;
        }
        candidate.theme_key = infer_theme_key(
            &candidate.name,
            candidate.fundamentals.as_ref(),
            &candidate.news,
        );
        if history_retrieval {
            let current_price = candidate.price.or(candidate.market_snapshot.current_price);
            candidate.history_match_snapshot = history_store
                .read_history(&candidate.symbol, &candidate.market, &candidate.theme_key, current_price)
                .await
                .with_context(|| format!("history retrieval failed for {}", candidate.symbol))?;
        }
    }

    score_candidates(&mut deep_pool);
    let deep_filtered: Vec<_> = if is_explicit_set {
        deep_pool
    } else {
        deep_pool
            .into_iter()
            .filter(|item| item.pass_filter)
            .collect()
    };
    let preselected = apply_portfolio_constraints(deep_filtered, pick_count);
    if preselected.is_empty() {
        anyhow::bail!("no winners remained after deep-stage evaluation");
    }

    let llm_selected = preselected
        .iter()
        .take(llm_review_limit)
        .cloned()
        .collect::<Vec<_>>();
    let prompt = build_prompt(
        MarketKind::from_market_str(&request.market).display_label(),
        &strategy,
        &analysis_date,
        &language,
        &llm_selected,
        &enriched,
    );
    // Query memory system for cross-ticker lessons
    let memory_log = crate::engine::memory::TradingMemoryLog::new(
        &std::env::var("DATA_DIR").unwrap_or_else(|_| "/data".to_string()),
        256,
    )
    .ok();

    let mut memory_context_parts = Vec::new();
    if let Some(ref mem) = memory_log {
        // For top candidates, get past context
        for candidate in preselected.iter().take(3) {
            if let Ok(bundle) = mem.past_context_bundle_async(&candidate.symbol, 3, 2).await
                && bundle.same_ticker_count > 0
            {
                let highlights_text = bundle
                    .same_ticker_highlights
                    .iter()
                    .take(2)
                    .map(|h| {
                        format!(
                            "{}: {}",
                            h.ticker,
                            h.summary.chars().take(80).collect::<String>()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                memory_context_parts.push(format!(
                    "Past analysis for {}: {} entries. {}",
                    candidate.symbol, bundle.same_ticker_count, highlights_text
                ));
            }
        }
    }

    let memory_context_str = if memory_context_parts.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n--- Memory Context ---\n{}",
            memory_context_parts.join("\n")
        )
    };

    let enriched_prompt = if guidance_context.is_empty() && memory_context_str.is_empty() {
        prompt
    } else {
        format!(
            "{}

--- Market Context ---
{}",
            prompt, guidance_context
        )
    };
    let content = llm_client
        .generate(&enriched_prompt)
        .await
        .context("failed to generate stock picks")?;
    let generated = parse_generated_stock_pick(&content)
        .with_context(|| format!("failed to parse stock pick JSON: {content}"))?;

    let selected_map = preselected
        .iter()
        .map(|item| (item.symbol.clone(), item.clone()))
        .collect::<HashMap<_, _>>();
    let explanation_map = generated
        .picks
        .into_iter()
        .map(|item| (item.symbol.trim().to_uppercase(), item))
        .collect::<HashMap<_, _>>();

    let deep_evaluated_count = preselected.len();
    let picks = preselected
        .into_iter()
        .map(|item| {
            let explanation = explanation_map.get(&item.symbol);
            let selection_reason_codes = explanation
                .map(|value| value.decision_reason_codes.clone())
                .filter(|codes| !codes.is_empty())
                .unwrap_or_else(|| default_selection_reason_codes(&item));
            let rejection_risk_flags = item.risk_snapshot.signal_codes.clone();
            let evidence_quality_score = score_evidence_quality(&item);
            let mut pick = StockPickItem {
                symbol: item.symbol.clone(),
                name: item.name.clone(),
                market: item.market.clone(),
                exchange: item.exchange.clone(),
                score: item.factor.total,
                confidence: explanation
                    .map(|value| llm::parse::normalize_probability(&value.confidence) * 100.0)
                    .unwrap_or((55.0 + item.factor.total * 0.35).clamp(0.0, 100.0)),
                thesis: explanation
                    .map(|value| value.thesis.clone())
                    .unwrap_or_else(|| default_thesis(&item)),
                catalysts: explanation
                    .map(|value| value.catalysts.clone())
                    .unwrap_or_else(|| default_catalysts(&item)),
                risks: explanation
                    .map(|value| value.risks.clone())
                    .unwrap_or_else(|| default_risks(&item)),
                evidence_points: explanation
                    .map(|value| value.evidence_points.clone())
                    .unwrap_or_else(|| default_evidence(&item)),
                price: item.price,
                change_pct: item.change_pct,
                market_cap: item.market_cap,
                priority_label: String::new(),
                priority_rank: 0,
                sort_key: 0.0,
                objective_assessment: StockPickObjectiveAssessment::default(),
                factor_breakdown: StockPickFactorBreakdown {
                    momentum: item.factor.momentum,
                    quality: item.factor.quality,
                    value: item.factor.value,
                    profitability: item.factor.profitability,
                    risk: item.factor.risk,
                    event: item.factor.event,
                    evidence: item.factor.evidence,
                    history: item.factor.history,
                    penalty: item.factor.penalty,
                    total: item.factor.total,
                },
                market_snapshot: item.market_snapshot.clone(),
                technical_snapshot: item.technical_snapshot.clone(),
                fundamental_snapshot: item.fundamental_snapshot.clone(),
                news_snapshot: item.news_snapshot.clone(),
                history_match_snapshot: item.history_match_snapshot.clone(),
                risk_snapshot: item.risk_snapshot.clone(),
                data_quality_snapshot: item.data_quality_snapshot.clone(),
                selection_reason_codes,
                rejection_risk_flags,
                evidence_quality_score,
            };
            pick.objective_assessment = evaluate_stock_pick_objective_assessment(&pick, &item);
            pick.priority_rank = stock_pick_priority_rank(&pick);
            pick.priority_label = stock_pick_priority_label(pick.priority_rank).to_string();
            pick.sort_key = stock_pick_sort_key(&pick);
            pick
        })
        .collect::<Vec<_>>();

    // Score each pick with the scoring system
    let score_config = crate::engine::score::config::ScoreConfig::from_env();
    let mut scored_picks = Vec::new();
    for pick in picks {
        let scoreable = crate::engine::score::scorer::ScoreablePick {
            symbol: pick.symbol.clone(),
            market: pick.market.clone(),
            rsi: pick.technical_snapshot.rsi,
            macd: pick.technical_snapshot.macd,
            macd_signal: pick.technical_snapshot.macd_signal,
            macd_hist: pick.technical_snapshot.macd_hist,
            adx: pick.technical_snapshot.adx,
            close_10_ema: pick.technical_snapshot.close_10_ema,
            close_50_sma: pick.technical_snapshot.close_50_sma,
            close_200_sma: pick.technical_snapshot.close_200_sma,
            obv: pick.technical_snapshot.obv,
            current_price: pick.price,
            volume_elevated: pick.market_snapshot.volume_ratio.map(|v| v > 1.2).unwrap_or(false),
            latest_positive: pick.change_pct.map(|c| c > 0.0).unwrap_or(false),
            pe_like: pick.fundamental_snapshot.pe_like,
            ps_like: pick.fundamental_snapshot.ps_like,
            roe: pick.fundamental_snapshot.roe,
            leverage: pick.fundamental_snapshot.leverage,
            market_cap: pick.market_cap,
            revenues_usd: pick.fundamental_snapshot.revenues_usd,
            net_income_usd: pick.fundamental_snapshot.net_income_usd,
            news_headlines: pick.news_snapshot.headline_titles.clone(),
            confidence: pick.confidence,
            objective_final_score: pick.objective_assessment.final_score as f64,
            momentum_score: pick.factor_breakdown.momentum,
            hit_rate: pick.history_match_snapshot.hit_rate,
            catalyst_count: pick.news_snapshot.catalyst_count,
            hard_negative_count: pick.news_snapshot.hard_negative_count,
            volume_ratio: pick.market_snapshot.volume_ratio,
            period_return_pct: pick.market_snapshot.period_return_pct,
        };
        let stock_score = crate::engine::score::score_stock_pick(llm_client, &scoreable, &score_config).await;
        tracing::info!(
            symbol = %pick.symbol,
            total = stock_score.total,
            technical = stock_score.technical.score,
            fundamental = stock_score.fundamental.score,
            sentiment = stock_score.sentiment.score,
            llm_analysis = stock_score.llm_analysis.score,
            "stock scored"
        );
        scored_picks.push((pick, stock_score));
    }

    // TODO: Store recommendations — previously used sa_storage::Store for PostgreSQL.
    // With trait-based architecture, this needs an AnalysisStore or similar trait.

    let picks: Vec<StockPickItem> = scored_picks
        .into_iter()
        .map(|(pick, _score)| {
            // TODO: Persist recommendation scores when storage trait is available
            pick
        })
        .collect();

    let explicit_rejected = generated.rejected_symbols;
    let filtered_rejected = enriched
        .iter()
        .filter(|item| !item.pass_filter)
        .map(|item| item.symbol.clone())
        .collect::<Vec<_>>();
    let constrained_rejected = selected_map.keys().cloned().collect::<HashSet<_>>();
    let residual_rejected = enriched
        .iter()
        .filter(|item| item.pass_filter && !constrained_rejected.contains(&item.symbol))
        .map(|item| item.symbol.clone())
        .collect::<Vec<_>>();

    let mut rejected_symbols = explicit_rejected;
    rejected_symbols.extend(filtered_rejected);
    rejected_symbols.extend(residual_rejected);
    rejected_symbols.sort();
    rejected_symbols.dedup();

    let history_match_summary = summarize_history_matches(&picks);
    let mut response = StockPickResponse {
        market: request.market.clone(),
        strategy,
        analysis_date,
        candidate_count: candidates.len(),
        evaluated_count: enriched.len(),
        coarse_candidate_count: candidates.len(),
        deep_evaluated_count,
        winner_count: picks.len(),
        objective_overview: summarize_stock_pick_objective_overview(&picks),
        picks,
        summary: generated.summary,
        rejected_symbols,
        selection_engine_version: "stock-pick-v2-dev".to_string(),
        selection_diagnostics: StockPickSelectionDiagnostics {
            search_depth: search_depth.to_string(),
            vector_store_enabled: true,
            redis_enabled: true,
            history_retrieval_enabled: history_retrieval,
            agreement_with_system_rank: generated.agreement_with_system_rank.clone(),
            override_count: generated.override_actions.len(),
        },
        evidence_coverage_summary: history_store.build_evidence_coverage_summary(
            candidates.len(),
            deep_evaluated_count,
            indexed_evidence_records,
            history_match_summary.sample_count,
        ),
        history_match_summary,
        storage_write_summary: StockPickStorageWriteSummary::default(),
        failure: None,
    };

    if generated
        .agreement_with_system_rank
        .trim()
        .eq_ignore_ascii_case("disagree")
    {
        if is_explicit_set {
            tracing::warn!(
                "llm review disagrees with system rank for explicit candidates, proceeding anyway"
            );
        } else {
            anyhow::bail!(
                "llm review disagrees with system rank without supported override integration"
            );
        }
    }

    let run_id = uuid::Uuid::new_v4().to_string();
    let theme_keys = response
        .picks
        .iter()
        .map(|pick| {
            (
                pick.symbol.clone(),
                pick.fundamental_snapshot
                    .industry
                    .clone()
                    .to_ascii_lowercase(),
            )
        })
        .collect::<Vec<_>>();
    let evidence_payloads = response
        .picks
        .iter()
        .flat_map(|pick| {
            let theme_key = pick.fundamental_snapshot.industry.to_ascii_lowercase();
            let symbol = pick.symbol.clone();
            let market = pick.market.clone();
            let analysis_date = response.analysis_date.clone();
            let records = selected_map
                .get(&pick.symbol)
                .map(|candidate| candidate.evidence_records.clone())
                .unwrap_or_default();
            records
                .into_iter()
                .map(move |record| StockPickEvidencePayload {
                    symbol: symbol.clone(),
                    market: market.clone(),
                    theme_key: theme_key.clone(),
                    analysis_date: analysis_date.clone(),
                    query: record.query,
                    published_at: record.published_at,
                    title: record.title,
                    summary: record.summary,
                    source: record.source,
                    url: record.url,
                    evidence_type: record.evidence_type,
                    sentiment_hint: record.sentiment_hint,
                    hard_negative_flag: record.hard_negative_flag,
                    dedupe_key: record.dedupe_key,
                })
        })
        .collect::<Vec<_>>();
    response.storage_write_summary = history_store
        .write_run(
            &run_id,
            &request.market,
            &response,
            &theme_keys,
            &evidence_payloads,
        )
        .await
        .context("failed to persist stock pick run")?;

    if target_output_mode == "focused" && response.picks.len() > 3 {
        response.picks.truncate(3);
        response.winner_count = response.picks.len();
    }

    Ok(response)
}

// ---------------------------------------------------------------------------
// Candidate resolution (inlined from candidates.rs)
// ---------------------------------------------------------------------------

async fn resolve_candidates(
    market_data: &MarketDataClient,
    request: &StockPickRequest,
    candidate_limit: usize,
) -> anyhow::Result<Vec<CandidateContext>> {
    if let Some(symbols) = request
        .candidate_symbols
        .as_ref()
        .filter(|items| !items.is_empty())
    {
        return Ok(symbols
            .iter()
            .map(|symbol| {
                let normalized = symbol.trim().to_uppercase();
                let market_kind = market_data.detect_market(&normalized);
                CandidateContext {
                    symbol: normalized.clone(),
                    name: normalized,
                    market: market_kind.display_label().to_string(),
                    exchange: market_kind.exchange_code().to_string(),
                    source_score: 0.0,
                }
            })
            .collect());
    }

    let market_kind = MarketKind::from_market_str(&request.market);
    match market_kind {
        MarketKind::AShare => {
            resolve_a_share_candidates(market_data, request, candidate_limit).await
        }
        MarketKind::HongKong => {
            let query = request
                .sector_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(market_kind.default_candidate_query());
            let items = market_data
                .search_stocks(
                    query,
                    Some(market_kind.display_label()),
                    candidate_limit,
                )
                .await?;
            Ok(items
                .into_iter()
                .map(|item| CandidateContext {
                    symbol: item.symbol,
                    name: item.name,
                    market: item.market,
                    exchange: item.exchange,
                    source_score: 0.0,
                })
                .collect())
        }
        MarketKind::UsEquity => {
            let query = request
                .sector_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(market_kind.default_candidate_query());
            let items = market_data
                .search_stocks(
                    query,
                    Some(market_kind.display_label()),
                    candidate_limit,
                )
                .await?;
            Ok(items
                .into_iter()
                .map(|item| CandidateContext {
                    symbol: item.symbol,
                    name: item.name,
                    market: item.market,
                    exchange: item.exchange,
                    source_score: 0.0,
                })
                .collect())
        }
    }
}

async fn resolve_a_share_candidates(
    market_data: &MarketDataClient,
    request: &StockPickRequest,
    candidate_limit: usize,
) -> anyhow::Result<Vec<CandidateContext>> {
    let preferred_sector_type = request.sector_type.as_deref().unwrap_or("industry");
    let secondary_sector_type = if preferred_sector_type == "industry" {
        "concept"
    } else {
        "industry"
    };

    let mut sector_types = vec![preferred_sector_type];
    if secondary_sector_type != preferred_sector_type {
        sector_types.push(secondary_sector_type);
    }

    let sector_limit = candidate_limit.clamp(6, 16);
    let per_sector_constituents = candidate_limit.clamp(5, 8);
    let mut ranked_sectors = Vec::new();

    for sector_type in sector_types {
        let sectors = market_data
            .fetch_a_share_sector_rankings(sector_type, sector_limit)
            .await
            .unwrap_or_default();

        let mut by_inflow = sectors.clone();
        by_inflow.sort_by(|left, right| {
            right
                .main_net_inflow
                .partial_cmp(&left.main_net_inflow)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    right
                        .change_pct
                        .partial_cmp(&left.change_pct)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        ranked_sectors.extend(by_inflow.into_iter().take(4));

        let mut by_change = sectors;
        by_change.sort_by(|left, right| {
            right
                .change_pct
                .partial_cmp(&left.change_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    right
                        .main_net_inflow
                        .partial_cmp(&left.main_net_inflow)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        ranked_sectors.extend(by_change.into_iter().take(4));
    }

    let mut sector_seen = HashSet::new();
    let mut sector_candidates = Vec::new();
    for sector in ranked_sectors {
        if !sector_seen.insert(sector.sector_code.clone()) {
            continue;
        }
        let constituents = market_data
            .fetch_a_share_sector_constituents(&sector.sector_code, per_sector_constituents)
            .await
            .unwrap_or_default();

        let mut by_inflow = constituents.clone();
        by_inflow.sort_by(|left, right| {
            right
                .main_net_inflow
                .unwrap_or_default()
                .partial_cmp(&left.main_net_inflow.unwrap_or_default())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    right
                        .change_pct
                        .partial_cmp(&left.change_pct)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        sector_candidates.extend(by_inflow.into_iter().take(3).map(|constituent| {
            CandidateContext {
                symbol: constituent.symbol,
                name: constituent.name,
                market: MarketKind::AShare.display_label().to_string(),
                exchange: MarketKind::AShare.exchange_code().to_string(),
                source_score: constituent.main_net_inflow.unwrap_or_default() / 1_0000_0000.0
                    + constituent.change_pct.max(0.0),
            }
        }));

        let mut by_change = constituents;
        by_change.sort_by(|left, right| {
            right
                .change_pct
                .partial_cmp(&left.change_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    right
                        .main_net_inflow
                        .unwrap_or_default()
                        .partial_cmp(&left.main_net_inflow.unwrap_or_default())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        sector_candidates.extend(by_change.into_iter().take(2).map(|constituent| {
            CandidateContext {
                symbol: constituent.symbol,
                name: constituent.name,
                market: MarketKind::AShare.display_label().to_string(),
                exchange: MarketKind::AShare.exchange_code().to_string(),
                source_score: constituent.change_pct
                    + constituent.main_net_inflow.unwrap_or_default() / 2_0000_0000.0,
            }
        }));
    }

    let mut search_candidates = Vec::new();
    for query in [
        "AI",
        "Robotics",
        "Semiconductors",
        "Innovative Pharma",
        "Banking",
        "Power",
        "Advanced Manufacturing",
        "Consumer Electronics",
    ] {
        let items = market_data
            .search_stocks(
                query,
                Some(MarketKind::AShare.display_label()),
                candidate_limit.clamp(5, 8),
            )
            .await
            .unwrap_or_default();
        search_candidates.extend(items.into_iter().map(|item| CandidateContext {
            symbol: item.symbol,
            name: item.name,
            market: item.market,
            exchange: item.exchange,
            source_score: 1.0,
        }));
    }

    let mut all_candidates = Vec::new();
    all_candidates.extend(sector_candidates);
    all_candidates.extend(search_candidates);
    let all_candidates = dedup_candidates(all_candidates, candidate_limit.saturating_mul(4));
    let shortlist = shortlist_a_share_candidates_for_flow(all_candidates, candidate_limit);
    Ok(
        pre_rank_a_share_candidates(market_data, shortlist, candidate_limit)
            .await
            .into_iter()
            .take(candidate_limit)
            .collect(),
    )
}

fn dedup_candidates(items: Vec<CandidateContext>, limit: usize) -> Vec<CandidateContext> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for item in items {
        if seen.insert(item.symbol.clone()) {
            output.push(item);
        }
        if output.len() >= limit {
            break;
        }
    }
    output
}

pub(crate) fn shortlist_a_share_candidates_for_flow(
    mut candidates: Vec<CandidateContext>,
    candidate_limit: usize,
) -> Vec<CandidateContext> {
    candidates.sort_by(|left, right| {
        right
            .source_score
            .partial_cmp(&left.source_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.symbol.cmp(&right.symbol))
    });

    let expensive_window = candidate_limit.saturating_mul(2).clamp(8, 18);
    candidates.truncate(expensive_window.min(candidates.len()));
    candidates
}

async fn pre_rank_a_share_candidates(
    market_data: &MarketDataClient,
    candidates: Vec<CandidateContext>,
    candidate_limit: usize,
) -> Vec<CandidateContext> {
    let mut ranked = stream::iter(candidates)
        .map(|candidate| {
            let market_data = market_data.clone();
            async move {
                let capital_flow = market_data
                    .fetch_capital_flow(&candidate.symbol, 2)
                    .await
                    .unwrap_or_default();
                let billboard = market_data
                    .fetch_billboard_entries(&candidate.symbol, 2)
                    .await
                    .unwrap_or_default();
                let score = candidate.source_score
                    + capital_flow_source_score(&capital_flow)
                    + billboard_source_score(&billboard);
                CandidateContext {
                    source_score: score,
                    ..candidate
                }
            }
        })
        .buffer_unordered(8)
        .collect::<Vec<_>>()
        .await;

    ranked.sort_by(|left, right| {
        right
            .source_score
            .partial_cmp(&left.source_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.symbol.cmp(&right.symbol))
    });
    dedup_candidates(ranked, candidate_limit)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_shortlist_a_share_candidates_for_flow(
    rows: Vec<(&str, f64)>,
    candidate_limit: usize,
) -> Vec<String> {
    shortlist_a_share_candidates_for_flow(
        rows.into_iter()
            .map(|(symbol, source_score)| CandidateContext {
                symbol: symbol.to_string(),
                name: symbol.to_string(),
                market: "A-share".to_string(),
                exchange: "CN".to_string(),
                source_score,
            })
            .collect(),
        candidate_limit,
    )
    .into_iter()
    .map(|item| item.symbol)
    .collect()
}

// ---------------------------------------------------------------------------
// Pipeline helpers (inlined from helpers.rs)
// ---------------------------------------------------------------------------


fn normalize_stock_pick_search_depth(value: Option<&str>) -> &'static str {
    match value
        .unwrap_or("standard")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "light" | "shallow" => "light",
        "deep" | "high" => "deep",
        _ => "standard",
    }
}

fn normalize_target_output_mode(value: Option<&str>) -> &'static str {
    match value
        .unwrap_or("focused")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "expanded" | "full" => "expanded",
        _ => "focused",
    }
}

fn derive_coarse_candidate_limit(candidate_limit: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => candidate_limit.clamp(6, 12),
        "deep" => candidate_limit.saturating_mul(2).clamp(12, 30),
        _ => candidate_limit.saturating_add(4).clamp(10, 24),
    }
}

fn derive_deep_candidate_limit(pick_count: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => pick_count.saturating_mul(2).clamp(3, 6),
        "deep" => pick_count.saturating_mul(4).clamp(6, 12),
        _ => pick_count.saturating_mul(3).clamp(4, 8),
    }
}

fn derive_llm_review_limit(pick_count: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => pick_count.clamp(1, 3),
        "deep" => pick_count.saturating_add(2).clamp(2, 5),
        _ => pick_count.saturating_add(1).clamp(2, 4),
    }
}

fn stock_pick_search_time_range(search_depth: &str) -> Option<&'static str> {
    match search_depth {
        "light" => Some("day"),
        "deep" => Some("week"),
        _ => None,
    }
}

fn build_light_search_queries(
    request: &StockPickRequest,
    candidates: &[CandidateContext],
) -> Vec<String> {
    let mut queries = Vec::new();
    let market_label = MarketKind::from_market_str(&request.market).display_label();
    if let Some(sector) = request
        .sector_type
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        queries.push(format!("{market_label} {sector} market outlook"));
        queries.push(format!("{market_label} {sector} earnings outlook"));
    }
    for candidate in candidates.iter().take(6) {
        let base_query = stock_pick_subject_query(&candidate.symbol, &candidate.name);
        queries.push(base_query.clone());
        queries.push(format!("{base_query} earnings"));
    }
    queries.sort();
    queries.dedup();
    queries
}

fn should_skip_light_stage_search(
    request: &StockPickRequest,
    candidates: &[CandidateContext],
) -> bool {
    request
        .candidate_symbols
        .as_ref()
        .is_some_and(|symbols| !symbols.is_empty() && symbols.len() == candidates.len())
}

fn build_candidate_search_queries(
    candidate: &EnrichedCandidate,
    request: &StockPickRequest,
) -> Vec<String> {
    let base_query = stock_pick_subject_query(&candidate.symbol, &candidate.name);
    let mut queries = vec![
        base_query.clone(),
        format!("{base_query} earnings"),
        format!("{base_query} guidance"),
    ];
    if !candidate.theme_key.trim().is_empty() && candidate.theme_key != "general" {
        queries.push(format!("{base_query} {} outlook", candidate.theme_key));
    }
    if let Some(strategy) = request
        .strategy
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        queries.push(format!("{base_query} {strategy}"));
    }
    queries.sort();
    queries.dedup();
    queries
}

fn stock_pick_subject_query(symbol: &str, name: &str) -> String {
    let normalized_symbol = symbol.trim();
    let normalized_name = name.trim();
    if normalized_name.is_empty() || normalized_name.eq_ignore_ascii_case(normalized_symbol) {
        return normalized_symbol.to_string();
    }
    format!("{normalized_name} {normalized_symbol}")
}

fn news_items_to_evidence_records(
    _symbol: &str,
    market: &str,
    theme_key: &str,
    queries: &[String],
    items: &[NewsItem],
) -> Vec<CandidateEvidenceRecord> {
    let query = queries.first().cloned().unwrap_or_default();
    let mut dedup = HashSet::new();
    let mut records = Vec::new();
    for item in items {
        let dedupe_key = crate::data::news::news_dedupe_key(
            &item.title,
            &item.source,
            &item.published_at,
            item.url.as_deref(),
        );
        if !dedup.insert(dedupe_key.clone()) {
            continue;
        }
        let sentiment = crate::data::news::classify_news_sentiment(&item.title, &item.summary);
        let hard_negative_flag = crate::data::news::is_hard_negative(item);
        let sentiment_hint = sentiment.as_str();
        records.push(CandidateEvidenceRecord {
            query: query.clone(),
            published_at: item.published_at.clone(),
            title: item.title.clone(),
            summary: item.summary.clone(),
            source: item.source.clone(),
            url: item.url.clone().unwrap_or_default(),
            evidence_type: if theme_key.trim().is_empty() {
                format!("{}_news", market.trim())
            } else {
                format!("{}_{}_news", market.trim(), theme_key.trim())
            },
            sentiment_hint: sentiment_hint.to_string(),
            hard_negative_flag,
            dedupe_key,
        });
    }
    records
}


fn default_selection_reason_codes(item: &EnrichedCandidate) -> Vec<String> {
    let mut codes = vec!["score_leader".to_string()];
    if item.factor.momentum >= 60.0 {
        codes.push("technical_support".to_string());
    }
    if item.factor.quality >= 60.0 || item.factor.profitability >= 60.0 {
        codes.push("fundamental_support".to_string());
    }
    if item.factor.evidence >= 55.0 {
        codes.push("evidence_support".to_string());
    }
    if item.factor.history >= 55.0 {
        codes.push("history_support".to_string());
    }
    if !item.risk_snapshot.signal_codes.is_empty() {
        codes.push("risk_capped".to_string());
    }
    codes
}

fn score_evidence_quality(item: &EnrichedCandidate) -> i32 {
    let source_score = item.news_snapshot.unique_source_count.min(5) as i32 * 10;
    let evidence_score = item.evidence_records.len().min(8) as i32 * 5;
    let history_score = if item.history_match_snapshot.sample_count > 0 {
        20
    } else {
        0
    };
    let penalty = item.news_snapshot.hard_negative_count.min(3) as i32 * 5;
    (source_score + evidence_score + history_score - penalty).clamp(0, 100)
}

fn summarize_history_matches(picks: &[StockPickItem]) -> StockPickHistoryMatchSnapshot {
    if picks.is_empty() {
        return StockPickHistoryMatchSnapshot::default();
    }
    let enabled = picks.iter().any(|pick| pick.history_match_snapshot.enabled);
    let sample_count = picks
        .iter()
        .map(|pick| pick.history_match_snapshot.sample_count)
        .sum::<usize>();
    let vector_hit_count = picks
        .iter()
        .map(|pick| pick.history_match_snapshot.vector_hit_count)
        .sum::<usize>();
    let average_score_values = picks
        .iter()
        .filter_map(|pick| pick.history_match_snapshot.average_score)
        .collect::<Vec<_>>();
    let hit_rate_values = picks
        .iter()
        .filter_map(|pick| pick.history_match_snapshot.hit_rate)
        .collect::<Vec<_>>();
    let alpha_values = picks
        .iter()
        .filter_map(|pick| pick.history_match_snapshot.average_alpha_return)
        .collect::<Vec<_>>();
    let top_matches = picks
        .iter()
        .flat_map(|pick| pick.history_match_snapshot.top_matches.clone())
        .take(12)
        .collect::<Vec<_>>();
    StockPickHistoryMatchSnapshot {
        enabled,
        sample_count,
        vector_hit_count,
        average_score: average_score_values
            .is_empty()
            .then_some(0.0)
            .filter(|_| false)
            .or_else(|| {
                (!average_score_values.is_empty()).then_some(
                    average_score_values.iter().sum::<f64>() / average_score_values.len() as f64,
                )
            }),
        hit_rate: (!hit_rate_values.is_empty())
            .then_some(hit_rate_values.iter().sum::<f64>() / hit_rate_values.len() as f64),
        average_alpha_return: (!alpha_values.is_empty())
            .then_some(alpha_values.iter().sum::<f64>() / alpha_values.len() as f64),
        top_matches,
    }
}

fn capital_flow_source_score(items: &[CapitalFlowPoint]) -> f64 {
    let Some(latest) = items.first().or_else(|| items.last()) else {
        return 0.0;
    };
    let hundred_million = 100_000_000.0;
    let inflow_component = (latest.main_net_inflow / hundred_million).clamp(-8.0, 12.0);
    let ratio_component = latest.main_net_inflow_ratio_pct.clamp(-10.0, 20.0) * 0.35;
    let price_component = latest.change_pct.clamp(-5.0, 12.0) * 0.5;
    inflow_component + ratio_component + price_component
}

fn billboard_source_score(items: &[BillboardEntry]) -> f64 {
    let Some(latest) = items.first().or_else(|| items.last()) else {
        return 0.0;
    };
    let net_component = latest
        .net_amount
        .map(|value| (value / 1_0000_0000.0).clamp(-6.0, 10.0))
        .unwrap_or(1.5);
    let turnover_component = latest
        .turnover_rate_pct
        .unwrap_or_default()
        .clamp(0.0, 30.0)
        * 0.15;
    let change_component = latest.change_rate_pct.clamp(-5.0, 12.0) * 0.4;
    net_component + turnover_component + change_component + 2.0
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_capital_flow_source_score(items: &[CapitalFlowPoint]) -> f64 {
    capital_flow_source_score(items)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_billboard_source_score(items: &[BillboardEntry]) -> f64 {
    billboard_source_score(items)
}


#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_shortlist_candidates_for_news(
    rows: Vec<(&str, f64)>,
    pick_count: usize,
) -> Vec<String> {
    use crate::engine::stock_pick::FactorBreakdown;
    use crate::models::{
        StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickMarketSnapshot,
        StockPickNewsSnapshot, StockPickRiskSnapshot, StockPickTechnicalSnapshot,
    };

    let mut shortlisted = crate::engine::stock_pick::scoring::shortlist_candidates_for_news(
        &rows
            .into_iter()
            .map(|(symbol, total)| EnrichedCandidate {
                symbol: symbol.to_string(),
                name: symbol.to_string(),
                market: "A-share".to_string(),
                exchange: "CN".to_string(),
                industry: "test".to_string(),
                price: Some(10.0),
                change_pct: Some(1.0),
                market_cap: Some(1_000_000_000.0),
                theme_key: "test".to_string(),
                fundamentals: None,
                news: Vec::new(),
                evidence_records: Vec::new(),
                candles: Vec::new(),
                technical_snapshot: StockPickTechnicalSnapshot::default(),
                market_snapshot: StockPickMarketSnapshot::default(),
                fundamental_snapshot: StockPickFundamentalSnapshot::default(),
                news_snapshot: StockPickNewsSnapshot::default(),
                history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
                risk_snapshot: StockPickRiskSnapshot::default(),
                data_quality_snapshot: StockPickDataQualitySnapshot::default(),
                factor: FactorBreakdown {
                    total,
                    ..FactorBreakdown::default()
                },
                pass_filter: true,
                rejected_reasons: Vec::new(),
                description: String::new(),
            })
            .collect::<Vec<_>>(),
        pick_count,
    )
    .into_iter()
    .collect::<Vec<_>>();
    shortlisted.sort();
    shortlisted
}

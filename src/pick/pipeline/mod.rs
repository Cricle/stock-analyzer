pub mod filter;
pub mod rank;
mod select;

use std::collections::{HashMap, HashSet};

use anyhow::Context;

use crate::data::MarketDataClient;
use crate::llm::{self as llm, LlmClient};
use crate::{
    StockPickFactorBreakdown, StockPickItem, StockPickObjectiveAssessment, StockPickRequest,
    StockPickResponse, StockPickSelectionDiagnostics, StockPickStorageWriteSummary,
};

use crate::pick::{
    CandidateContext, CandidateEvidenceRecord, EnrichedCandidate, StockPickEvidencePayload,
    StockPickHistoryStore, parse_generated_stock_pick,
};

use crate::pick::validation::{PickQualityGate, validate_and_enhance_picks};

use crate::pick::{
    apply_portfolio_constraints, enrich_candidates, infer_theme_key, score_candidates,
};

use crate::pick::objective::{
    build_prompt, default_catalysts, default_evidence, default_risks, default_thesis,
    evaluate_stock_pick_objective_assessment, stock_pick_priority_label, stock_pick_priority_rank,
    stock_pick_sort_key, summarize_stock_pick_objective_overview,
};

use filter::{market_display_label, market_kind_from_value, resolve_candidates};
use rank::{
    dedupe_news_items, default_selection_reason_codes, filter_relevant_news,
    news_items_to_evidence_records, score_evidence_quality, summarize_history_matches,
};
use select::{
    build_candidate_search_queries, build_light_search_queries, deep_search_limit,
    derive_coarse_candidate_limit, derive_deep_candidate_limit, derive_llm_review_limit,
    normalize_stock_pick_search_depth, normalize_target_output_mode,
    should_skip_light_stage_search, stock_pick_search_time_range,
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
    let guidance_context = match crate::guide::GuidanceStore::from_env()
        .get_latest_stock_pick_summary(&request.market)
        .await
    {
        Ok(Some(summary)) => {
            let sentiment = summary
                .get("market_sentiment")
                .and_then(|v| v.get("label"))
                .and_then(|v| v.as_str())
                .unwrap_or("neutral");
            let sentiment_score = summary
                .get("market_sentiment")
                .and_then(|v| v.get("score"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let sector_highlights = summary
                .get("sector_highlights")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|s| {
                            let name = s.get("sector_name")?.as_str()?;
                            let direction = s.get("direction")?.as_str()?;
                            Some(format!("{}: {}", name, direction))
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();

            let risk_alerts = summary
                .get("risk_alerts")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| {
                            let severity = a.get("severity")?.as_str()?;
                            let category = a.get("category")?.as_str()?;
                            Some(format!("{}: {}", severity, category))
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();

            let recent_picks = summary
                .get("picks")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|p| p.get("symbol").and_then(|v| v.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();

            format!(
                "Market sentiment: {} (score: {})\n\
                 Sector highlights: {}\n\
                 Risk alerts: {}\n\
                 Recent picks: {}",
                sentiment, sentiment_score, sector_highlights, risk_alerts, recent_picks
            )
        }
        _ => String::new(),
    };

    let candidates = resolve_candidates(market_data, request, coarse_candidate_limit).await?;
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
    let _light_evidence = if light_queries.is_empty() {
        Vec::new()
    } else {
        market_data
            .fetch_news_search_evidence(
                &light_queries.iter().map(String::as_str).collect::<Vec<_>>(),
                &language,
                search_time_range,
                coarse_candidate_limit.saturating_mul(4),
            )
            .await
            .context("light-stage search evidence fetch failed")?
    };

    let mut enriched = enrich_candidates(market_data, &candidates, deep_candidate_limit).await;

    // Build provenance for each enriched candidate
    for candidate in &mut enriched {
        candidate.provenance = build_candidate_provenance(candidate, &analysis_date);
    }

    score_candidates(&mut enriched);

    let filtered = enriched
        .iter()
        .filter(|item| item.pass_filter)
        .cloned()
        .collect::<Vec<_>>();
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
        let search_items = market_data
            .fetch_news_search_evidence(
                &deep_queries.iter().map(String::as_str).collect::<Vec<_>>(),
                &language,
                search_time_range,
                deep_search_limit(search_depth),
            )
            .await
            .with_context(|| {
                format!(
                    "deep-stage search evidence fetch failed for {}",
                    candidate.symbol
                )
            })?;
        if search_items.is_empty() {
            anyhow::bail!("missing deep-stage evidence for {}", candidate.symbol);
        }
        let deduped_records = news_items_to_evidence_records(
            &candidate.symbol,
            &candidate.market,
            &candidate.theme_key,
            &deep_queries,
            &search_items,
        );
        if deduped_records.is_empty() {
            anyhow::bail!("missing structured deep evidence for {}", candidate.symbol);
        }
        indexed_evidence_records += deduped_records.len();
        candidate.news = dedupe_news_items(filter_relevant_news(
            candidate.news.iter().cloned().chain(search_items).collect(),
            &candidate.symbol,
            &candidate.name,
        ));
        candidate.evidence_records = deduped_records;
        candidate.theme_key = infer_theme_key(
            &candidate.name,
            candidate.fundamentals.as_ref(),
            &candidate.news,
        );
        if history_retrieval {
            let current_price = candidate.price.or(candidate.market_snapshot.current_price);
            candidate.history_match_snapshot = history_store
                .read_history(
                    &candidate.symbol,
                    &candidate.market,
                    &candidate.theme_key,
                    current_price,
                )
                .await
                .with_context(|| format!("history retrieval failed for {}", candidate.symbol))?;
        }
    }

    score_candidates(&mut deep_pool);
    let preselected = apply_portfolio_constraints(
        deep_pool
            .into_iter()
            .filter(|item| item.pass_filter)
            .collect::<Vec<_>>(),
        pick_count,
    );
    if preselected.is_empty() {
        anyhow::bail!("no winners remained after deep-stage evaluation");
    }

    let llm_selected = preselected
        .iter()
        .take(llm_review_limit)
        .cloned()
        .collect::<Vec<_>>();
    let prompt = build_prompt(
        market_display_label(market_kind_from_value(&request.market)),
        &strategy,
        &analysis_date,
        &language,
        &llm_selected,
        &enriched,
    );
    // Query memory system for cross-ticker lessons
    let memory_log = crate::memory::TradingMemoryLog::new(
        &std::env::var("DATA_DIR").unwrap_or_else(|_| "/data".to_string()),
        256,
    )
    .ok();

    let mut memory_context_parts = Vec::with_capacity(4);
    if let Some(ref mem) = memory_log {
        // Get cross-ticker lessons for this market
        if let Ok(lessons) = mem.cross_ticker_lessons(&request.market, &[], 3).await
            && !lessons.is_empty()
        {
            let lessons_text = lessons
                .iter()
                .map(|l| format!("- {} (rating: {})", l.summary, l.rating))
                .collect::<Vec<_>>()
                .join("\n");
            memory_context_parts.push(format!("Cross-ticker lessons:\n{}", lessons_text));
        }

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
    tracing::info!(
        prompt_len = enriched_prompt.len(),
        "calling LLM for stock picks"
    );
    let content = llm_client
        .generate(&enriched_prompt)
        .await
        .context("failed to generate stock picks")?;
    tracing::info!(
        content_len = content.len(),
        "LLM stock pick raw output received"
    );
    let generated = parse_generated_stock_pick(&content)
        .with_context(|| format!("failed to parse stock pick JSON: {content}"))?;
    tracing::info!(
        pick_count = generated.picks.len(),
        summary = %generated.summary,
        "LLM stock pick parsed"
    );
    for pick in &generated.picks {
        tracing::info!(
            symbol = %pick.symbol,
            thesis_key = %pick.thesis.key,
            thesis_params = ?pick.thesis.params,
            catalysts_count = pick.catalysts.len(),
            risks_count = pick.risks.len(),
            "parsed LLM stock pick"
        );
    }

    // Validate and enhance picks with actionable defaults
    let quality_gate = PickQualityGate::default();
    let validated_picks = validate_and_enhance_picks(generated.picks, &preselected, &quality_gate);

    let generated = crate::pick::types::GeneratedStockPickResponse {
        picks: validated_picks,
        ..generated
    };

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
                    .filter(|t| !t.key.trim().is_empty())
                    .unwrap_or_else(|| default_thesis(&item)),
                catalysts: explanation
                    .map(|value| value.catalysts.clone())
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| default_catalysts(&item)),
                risks: explanation
                    .map(|value| value.risks.clone())
                    .filter(|r| !r.is_empty())
                    .unwrap_or_else(|| default_risks(&item)),
                evidence_points: explanation
                    .map(|value| value.evidence_points.clone())
                    .unwrap_or_else(|| default_evidence(&item)),
                entry_price: explanation.and_then(|value| value.entry_price.clone()),
                entry_rationale: explanation.and_then(|value| value.entry_rationale.clone()),
                stop_loss: explanation.and_then(|value| value.stop_loss.clone()),
                stop_rationale: explanation.and_then(|value| value.stop_rationale.clone()),
                target_price: explanation.and_then(|value| value.target_price.clone()),
                target_rationale: explanation.and_then(|value| value.target_rationale.clone()),
                holding_period: explanation.and_then(|value| value.holding_period.clone()),
                exit_triggers: explanation
                    .map(|value| value.exit_triggers.clone())
                    .unwrap_or_default(),
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
                provenance_snapshot: item.provenance.clone(),
                quality_tier: crate::pick::enrichment::StockPickQualityTier::default(),
                enrichment_attempt: None,
            };
            // Apply actionable field defaults if LLM didn't provide them
            let current_price = item.price.or(item.market_snapshot.current_price);
            let atr = item.technical_snapshot.atr;
            // Validate price fields are numeric, reset if free text
            if let Some(ref ep) = pick.entry_price
                && ep.parse::<f64>().is_err()
                && !ep.contains('-')
            {
                pick.entry_price = None;
            }
            if let Some(ref sl) = pick.stop_loss
                && sl.parse::<f64>().is_err()
            {
                pick.stop_loss = None;
            }
            if let Some(ref tp) = pick.target_price
                && tp.parse::<f64>().is_err()
                && !tp.contains('-')
            {
                pick.target_price = None;
            }
            if pick.entry_price.is_none()
                && let Some(price) = current_price
            {
                pick.entry_price = Some(format!("{:.2}", price));
            }
            if pick.stop_loss.is_none()
                && let Some(entry_str) = &pick.entry_price
                && let Ok(entry) = entry_str.parse::<f64>()
            {
                let stop = if let Some(atr_val) = atr {
                    entry - 2.0 * atr_val
                } else {
                    entry * 0.95
                };
                // Cap stop loss at 10% below entry
                let max_stop = entry * 0.90;
                pick.stop_loss = Some(format!("{:.2}", stop.max(max_stop).max(0.01)));
            }
            if pick.target_price.is_none()
                && let (Some(entry_str), Some(stop_str)) = (&pick.entry_price, &pick.stop_loss)
                && let (Ok(entry), Ok(stop)) = (entry_str.parse::<f64>(), stop_str.parse::<f64>())
            {
                let risk = (entry - stop).abs();
                if risk > 0.0 {
                    let target = entry + 3.0 * risk;
                    pick.target_price = Some(format!("{:.2}", target));
                }
            }
            if pick.holding_period.is_none() {
                pick.holding_period = Some("2-4 weeks".to_string());
            }
            if pick.exit_triggers.is_empty()
                && let Some(stop_str) = &pick.stop_loss
            {
                pick.exit_triggers.push(format!("break below {}", stop_str));
            }
            pick.objective_assessment = evaluate_stock_pick_objective_assessment(&pick, &item);
            pick.quality_tier = crate::pick::enrichment::classify_quality_tier(&pick.objective_assessment);
            pick.priority_rank = stock_pick_priority_rank(&pick);
            pick.priority_label = stock_pick_priority_label(pick.priority_rank).to_string();
            pick.sort_key = stock_pick_sort_key(&pick);
            pick
        })
        .collect::<Vec<_>>();

    // Attempt enrichment for insufficient picks (stub for now)
    let mut picks = picks;
    for pick in &mut picks {
        if matches!(pick.quality_tier, crate::pick::enrichment::StockPickQualityTier::DataInsufficient) {
            tracing::info!("Pick {} classified as DataInsufficient, enrichment not yet implemented", pick.symbol);
            pick.enrichment_attempt = Some(crate::pick::enrichment::EnrichmentAttempt {
                attempted_at: chrono::Utc::now().to_rfc3339(),
                target_fields: vec![],
                success: false,
                error: Some("enrichment not implemented".to_string()),
            });
        }
    }

    // Filter out picks that are not ready (missing market_cap or low objective score)
    let picks: Vec<_> = picks
        .into_iter()
        .filter(|pick| {
            if !pick.objective_assessment.ready {
                tracing::warn!(
                    symbol = %pick.symbol,
                    score = pick.objective_assessment.final_score,
                    "pick filtered: not ready (missing market_cap or low score)"
                );
                false
            } else {
                true
            }
        })
        .collect();

    // Score each pick with the scoring system
    let score_config = crate::config::SaConfig::load().score_config();
    let mut scored_picks = Vec::with_capacity(picks.len());
    for pick in picks {
        let scoreable = crate::scoring::scorer::ScoreablePick {
            symbol: pick.symbol.clone(),
            market: pick.market.clone(),
            technical: crate::scoring::dimensions::technical::TechnicalInput {
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
                volume_elevated: pick
                    .market_snapshot
                    .volume_ratio
                    .map(|v| v > 1.2)
                    .unwrap_or(false),
                latest_positive: pick.change_pct.map(|c| c > 0.0).unwrap_or(false),
            },
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
        let stock_score =
            crate::scoring::score_stock_pick(llm_client, &scoreable, &score_config).await;
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

    // TODO: Store recommendations via AnalysisStore trait.

    let picks: Vec<StockPickItem> = scored_picks
        .into_iter()
        .map(|(pick, _score)| {
            // TODO: Persist recommendation scores via AnalysisStore trait.
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
            history_retrieval_enabled: history_retrieval,
            agreement_with_system_rank: generated.agreement_with_system_rank.to_string(),
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

    if generated.agreement_with_system_rank == crate::pick::types::AgreementLevel::Disagree {
        tracing::warn!(
            agreement = %generated.agreement_with_system_rank,
            override_count = generated.override_actions.len(),
            "llm review disagrees with system rank; continuing with LLM picks"
        );
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

fn build_candidate_provenance(
    candidate: &crate::pick::EnrichedCandidate,
    analysis_date: &str,
) -> crate::pick::ProvenanceSnapshot {
    use crate::pick::{DataProvenance, ProvenanceSnapshot};

    let market_data = if candidate.price.is_some() || candidate.market_cap.is_some() {
        Some(DataProvenance {
            source: "market_data_client".to_string(),
            fetched_at: analysis_date.to_string(),
            confidence: 0.9,
            field_coverage: vec!["price".to_string(), "market_cap".to_string()],
        })
    } else {
        None
    };

    let fundamentals = candidate.fundamentals.as_ref().map(|_| DataProvenance {
        source: "fundamentals_provider".to_string(),
        fetched_at: analysis_date.to_string(),
        confidence: 0.85,
        field_coverage: vec!["income".to_string(), "balance".to_string()],
    });

    let technicals = if !candidate.candles.is_empty() {
        Some(DataProvenance {
            source: "computed_from_candles".to_string(),
            fetched_at: analysis_date.to_string(),
            confidence: (candidate.candles.len() as f64 / 30.0).min(1.0),
            field_coverage: vec!["rsi".to_string(), "macd".to_string()],
        })
    } else {
        None
    };

    let news = if !candidate.news.is_empty() {
        Some(DataProvenance {
            source: "news_provider".to_string(),
            fetched_at: analysis_date.to_string(),
            confidence: (candidate.news.len() as f64 / 5.0).min(1.0),
            field_coverage: vec!["articles".to_string()],
        })
    } else {
        None
    };

    ProvenanceSnapshot {
        market_data,
        fundamentals,
        technicals,
        news,
    }
}

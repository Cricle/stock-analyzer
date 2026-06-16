use std::collections::{HashMap, HashSet};

use futures::{StreamExt, stream};

use crate::data::{BillboardEntry, CapitalFlowPoint, MarketDataClient, MarketKind, NewsItem};
use crate::models::{StockPickHistoryMatchSnapshot, StockPickItem, StockPickRequest};

use crate::engine::stock_pick::{
    CandidateContext, CandidateEvidenceRecord, EnrichedCandidate,
};

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

pub(crate) async fn pre_rank_a_share_candidates(
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

// ---------------------------------------------------------------------------
// Pipeline helpers
// ---------------------------------------------------------------------------

pub(crate) fn normalize_stock_pick_search_depth(value: Option<&str>) -> &'static str {
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

pub(crate) fn normalize_target_output_mode(value: Option<&str>) -> &'static str {
    match value
        .unwrap_or("scored")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "minimal" | "min" => "minimal",
        "full" | "detailed" => "full",
        _ => "scored",
    }
}

pub(crate) fn derive_coarse_candidate_limit(candidate_limit: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => candidate_limit.saturating_mul(2).clamp(8, 24),
        "deep" => candidate_limit.saturating_mul(3).clamp(12, 36),
        _ => candidate_limit.saturating_mul(2).clamp(8, 30),
    }
}

pub(crate) fn derive_deep_candidate_limit(pick_count: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => pick_count.saturating_mul(2).clamp(2, 4),
        "deep" => pick_count.saturating_mul(4).clamp(4, 8),
        _ => pick_count.saturating_add(1).clamp(2, 6),
    }
}

pub(crate) fn derive_llm_review_limit(pick_count: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => pick_count.saturating_add(1).clamp(2, 3),
        "deep" => pick_count.saturating_add(2).clamp(3, 5),
        _ => pick_count.saturating_add(1).clamp(2, 4),
    }
}

pub(crate) fn stock_pick_search_time_range(search_depth: &str) -> Option<&'static str> {
    match search_depth {
        "light" => Some("day"),
        "deep" => Some("week"),
        _ => None,
    }
}

pub(crate) fn build_light_search_queries(
    request: &StockPickRequest,
    _candidates: &[CandidateContext],
) -> Vec<String> {
    let mut queries = Vec::new();
    let market_label = MarketKind::from_market_str(&request.market).display_label();
    if let Some(sector) = request
        .sector_type
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        queries.push(format!("{market_label} {sector} sector"));
    }
    let base_query = format!("{market_label} stock pick");
    queries.push(base_query.clone());
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

pub(crate) fn should_skip_light_stage_search(
    request: &StockPickRequest,
    _candidates: &[CandidateContext],
) -> bool {
    request
        .candidate_symbols
        .as_ref()
        .is_some_and(|items| !items.is_empty())
}

pub(crate) fn build_candidate_search_queries(
    candidate: &EnrichedCandidate,
    request: &StockPickRequest,
) -> Vec<String> {
    let mut queries = Vec::new();
    let subject = stock_pick_subject_query(&candidate.symbol, &candidate.name);
    let market_label = MarketKind::from_market_str(&request.market).display_label();
    queries.push(format!("{subject} {market_label}"));
    if let Some(strategy) = request
        .strategy
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        queries.push(format!("{subject} {strategy}"));
    }
    queries.sort();
    queries.dedup();
    queries
}

pub(crate) fn stock_pick_subject_query(symbol: &str, name: &str) -> String {
    let normalized_symbol = symbol.trim();
    let normalized_name = name.trim();
    if normalized_name.is_empty() || normalized_name.eq_ignore_ascii_case(normalized_symbol) {
        return normalized_symbol.to_string();
    }
    format!("{normalized_name} {normalized_symbol}")
}

pub(crate) fn news_items_to_evidence_records(
    _symbol: &str,
    market: &str,
    theme_key: &str,
    queries: &[String],
    items: &[NewsItem],
    sentiment_map: Option<&HashMap<String, String>>,
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
        // Use LLM-classified sentiment; default to "neutral" if LLM didn't classify
        let sentiment_hint = sentiment_map
            .and_then(|m| m.get(&dedupe_key).cloned())
            .unwrap_or_else(|| "neutral".to_string());
        let hard_negative_flag = sentiment_hint == "negative";
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
            sentiment_hint,
            hard_negative_flag,
            dedupe_key,
        });
    }
    records
}

/// Batch-classify news sentiment using LLM. Returns a map of dedupe_key → sentiment.
pub(crate) async fn classify_evidence_news_sentiment(
    llm: &crate::engine::llm::LlmClient,
    candidate_news: &HashMap<usize, Vec<NewsItem>>,
) -> anyhow::Result<HashMap<String, String>> {
    // Collect all news items with their dedupe keys
    let mut items_with_keys: Vec<(String, &NewsItem)> = Vec::new();
    for items in candidate_news.values() {
        for item in items {
            let key = crate::data::news::news_dedupe_key(
                &item.title,
                &item.source,
                &item.published_at,
                item.url.as_deref(),
            );
            items_with_keys.push((key, item));
        }
    }
    if items_with_keys.is_empty() {
        return Ok(HashMap::new());
    }

    // Limit to 30 items to avoid token overflow
    items_with_keys.truncate(30);

    let items_json: Vec<serde_json::Value> = items_with_keys
        .iter()
        .enumerate()
        .map(|(idx, (_, item))| {
            serde_json::json!({
                "id": idx,
                "title": item.title,
                "summary": item.summary,
            })
        })
        .collect();

    let prompt = format!(
        r#"Classify each news item's sentiment for stock investment analysis.
Return ONLY a JSON array: [{{"id":0,"sentiment":"positive"}}]

Sentiment values:
- "positive": earnings beat, upgrade, stimulus, rally, record high, buyback, strong growth, net institutional buying
- "negative": earnings miss, downgrade, scandal, fraud, fine, delisting, decline, layoffs, 增长放缓
- "neutral": no clear directional impact on stock price

News items:
{}"#,
        serde_json::to_string_pretty(&items_json)?
    );

    let response = llm.generate(&prompt).await?;

    let json_str = response
        .trim()
        .strip_prefix("```json")
        .or_else(|| response.trim().strip_prefix("```"))
        .unwrap_or(response.trim())
        .strip_suffix("```")
        .unwrap_or(response.trim())
        .trim();

    let json_str = if json_str.starts_with('[') {
        json_str
    } else if let Some(start) = json_str.find('[') {
        if let Some(end) = json_str.rfind(']') {
            &json_str[start..=end]
        } else {
            json_str
        }
    } else {
        json_str
    };

    let parsed: Vec<serde_json::Value> = serde_json::from_str(json_str)
        .map_err(|e| {
            tracing::warn!(response = %response.chars().take(200).collect::<String>(), "LLM evidence sentiment raw response");
            anyhow::anyhow!("failed to parse LLM evidence sentiment JSON: {e}")
        })?;

    let mut sentiment_map = HashMap::new();
    for entry in &parsed {
        let Some(id) = entry.get("id").and_then(|v| v.as_u64()) else {
            continue;
        };
        let idx = id as usize;
        if idx >= items_with_keys.len() {
            continue;
        }
        if let Some(sentiment) = entry.get("sentiment").and_then(|v| v.as_str())
            && matches!(sentiment, "positive" | "negative" | "neutral")
        {
            sentiment_map.insert(items_with_keys[idx].0.clone(), sentiment.to_string());
        }
    }

    tracing::info!(
        total = items_with_keys.len(),
        classified = sentiment_map.len(),
        positive = sentiment_map.values().filter(|v| v.as_str() == "positive").count(),
        negative = sentiment_map.values().filter(|v| v.as_str() == "negative").count(),
        "LLM evidence sentiment classification applied"
    );

    Ok(sentiment_map)
}

/// Parse LLM JSON response, stripping markdown fences if present.
pub(crate) fn default_selection_reason_codes(item: &EnrichedCandidate) -> Vec<String> {
    let mut codes = vec!["score_leader".to_string()];
    if item.factor.momentum >= 60.0 {
        codes.push("technical_support".to_string());
    }
    if item.factor.quality >= 60.0 || item.factor.profitability >= 60.0 {
        codes.push("fundamental_support".to_string());
    }
    if item.factor.growth >= 60.0 {
        codes.push("growth_support".to_string());
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
    // Enrichment-based reason codes
    if item.fundamental_snapshot.pe_ttm.is_some_and(|v| v > 0.0 && v < 25.0) {
        codes.push("valuation_support".to_string());
    }
    if item.fundamental_snapshot.analyst_buy_ratio.is_some_and(|v| v > 0.6) {
        codes.push("analyst_consensus".to_string());
    }
    if item.fundamental_snapshot.fund_flow_net_ratio.is_some_and(|v| v > 0.03) {
        codes.push("fund_flow_support".to_string());
    }
    if item.fundamental_snapshot.dividend_yield.is_some_and(|v| v > 0.02) {
        codes.push("income_support".to_string());
    }
    codes
}

pub(crate) fn score_evidence_quality(item: &EnrichedCandidate) -> i32 {
    let source_score = item.news_snapshot.unique_source_count.min(5) as i32 * 10;
    let evidence_score = item.evidence_records.len().min(8) as i32 * 5;
    let history_score = if item.history_match_snapshot.sample_count > 0 {
        20
    } else {
        0
    };
    // Enrichment data bonus
    let enrichment_score = [
        item.enrichment.pe_ttm.is_some(),
        item.enrichment.pb.is_some(),
        item.enrichment.revenue_yoy.is_some(),
        item.enrichment.net_profit_yoy.is_some(),
        item.enrichment.fund_flow_net_ratio.is_some(),
        item.enrichment.analyst_report_count.is_some(),
        item.enrichment.gross_margin.is_some(),
        item.enrichment.dividend_yield.is_some(),
        item.enrichment.chip_benefit_ratio.is_some(),
    ]
    .iter()
    .filter(|v| **v)
    .count() as i32
        * 4;
    let penalty = item.news_snapshot.hard_negative_count.min(3) as i32 * 5;
    (source_score + evidence_score + history_score + enrichment_score - penalty).clamp(0, 100)
}

pub(crate) fn summarize_history_matches(picks: &[StockPickItem]) -> StockPickHistoryMatchSnapshot {
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
        .collect::<Vec<_>>();
    StockPickHistoryMatchSnapshot {
        enabled,
        sample_count,
        vector_hit_count,
        average_score: average_average(&average_score_values),
        hit_rate: average_average(&hit_rate_values),
        average_alpha_return: average_average(&alpha_values),
        top_matches,
    }
}

fn average_average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

pub(crate) fn capital_flow_source_score(items: &[CapitalFlowPoint]) -> f64 {
    if items.is_empty() {
        return 0.0;
    }
    let latest = &items[0];
    let net_component = (latest.main_net_inflow / 1_0000_0000.0).clamp(-6.0, 10.0);
    let ratio_component = latest.main_net_inflow_ratio_pct.clamp(-30.0, 30.0) * 0.2;
    let change_component = latest.change_pct.clamp(-5.0, 12.0) * 0.4;
    net_component + ratio_component + change_component + 2.0
}

pub(crate) fn billboard_source_score(items: &[BillboardEntry]) -> f64 {
    if items.is_empty() {
        return 0.0;
    }
    let total_net: f64 = items
        .iter()
        .filter_map(|e| e.net_amount)
        .sum();
    let score = (total_net / 1_0000_0000.0).clamp(-5.0, 10.0);
    score + 3.0
}

pub(crate) fn dedup_candidates(items: Vec<CandidateContext>, limit: usize) -> Vec<CandidateContext> {
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


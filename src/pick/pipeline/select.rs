use crate::StockPickRequest;

use super::CandidateContext;
use super::filter::{market_display_label, market_kind_from_value};

// ---------------------------------------------------------------------------
// Pipeline configuration
// ---------------------------------------------------------------------------

pub(super) fn normalize_stock_pick_search_depth(value: Option<&str>) -> &'static str {
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

pub(super) fn normalize_target_output_mode(value: Option<&str>) -> &'static str {
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

pub(super) fn derive_coarse_candidate_limit(candidate_limit: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => candidate_limit.clamp(6, 12),
        "deep" => candidate_limit.saturating_mul(2).clamp(12, 30),
        _ => candidate_limit.saturating_add(4).clamp(10, 24),
    }
}

pub(super) fn derive_deep_candidate_limit(pick_count: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => pick_count.saturating_mul(2).clamp(3, 6),
        "deep" => pick_count.saturating_mul(4).clamp(6, 12),
        _ => pick_count.saturating_mul(3).clamp(4, 8),
    }
}

pub(super) fn derive_llm_review_limit(pick_count: usize, search_depth: &str) -> usize {
    match search_depth {
        "light" => pick_count.clamp(1, 3),
        "deep" => pick_count.saturating_add(2).clamp(2, 5),
        _ => pick_count.saturating_add(1).clamp(2, 4),
    }
}

pub(super) fn stock_pick_search_time_range(search_depth: &str) -> Option<&'static str> {
    match search_depth {
        "light" => Some("day"),
        "deep" => Some("week"),
        _ => None,
    }
}

pub(super) fn deep_search_limit(search_depth: &str) -> usize {
    match search_depth {
        "light" => 6,
        "deep" => 16,
        _ => 10,
    }
}

// ---------------------------------------------------------------------------
// Search query builders
// ---------------------------------------------------------------------------

pub(super) fn build_light_search_queries(
    request: &StockPickRequest,
    candidates: &[CandidateContext],
) -> Vec<String> {
    let mut queries = Vec::new();
    let market_label = market_display_label(market_kind_from_value(&request.market));
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

pub(super) fn should_skip_light_stage_search(
    request: &StockPickRequest,
    candidates: &[CandidateContext],
) -> bool {
    request
        .candidate_symbols
        .as_ref()
        .is_some_and(|symbols| !symbols.is_empty() && symbols.len() == candidates.len())
}

pub(super) fn build_candidate_search_queries(
    candidate: &super::EnrichedCandidate,
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

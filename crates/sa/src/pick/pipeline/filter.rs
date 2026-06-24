use std::collections::HashSet;

use futures::{StreamExt, stream};

use crate::data::{BillboardEntry, CapitalFlowPoint, MarketDataClient, MarketKind};
use crate::StockPickRequest;

use super::CandidateContext;

// ---------------------------------------------------------------------------
// Market helpers
// ---------------------------------------------------------------------------

pub(super) fn market_kind_from_value(value: &str) -> MarketKind {
    match value.trim().to_ascii_lowercase().as_str() {
        "a" | "a-share" | "a_share" | "ashare" | "cn" | "china" | "a股" => MarketKind::AShare,
        "hk" | "hkex" | "hongkong" | "hong_kong" | "港股" => MarketKind::HongKong,
        _ => MarketKind::UsEquity,
    }
}

pub(super) fn market_display_label(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "A-share",
        MarketKind::HongKong => "HK",
        MarketKind::UsEquity => "US",
    }
}

pub(super) fn market_search_label(market: MarketKind) -> &'static str {
    market_display_label(market)
}

pub(super) fn market_exchange_code(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "CN",
        MarketKind::HongKong => "HK",
        MarketKind::UsEquity => "US",
    }
}

fn default_market_candidate_query(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "industry",
        MarketKind::HongKong => "blue chip",
        MarketKind::UsEquity => "technology",
    }
}

// ---------------------------------------------------------------------------
// Candidate resolution
// ---------------------------------------------------------------------------

pub(super) async fn resolve_candidates(
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
                    market: market_display_label(market_kind).to_string(),
                    exchange: market_exchange_code(market_kind).to_string(),
                    source_score: 0.0,
                }
            })
            .collect());
    }

    let market_kind = market_kind_from_value(&request.market);
    match market_kind {
        MarketKind::AShare => {
            resolve_a_share_candidates(market_data, request, candidate_limit).await
        }
        MarketKind::HongKong => {
            let query = request
                .sector_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(default_market_candidate_query(market_kind));
            let items = market_data
                .search_stocks(
                    query,
                    Some(market_search_label(market_kind)),
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
                .unwrap_or(default_market_candidate_query(market_kind));
            let items = market_data
                .search_stocks(
                    query,
                    Some(market_search_label(market_kind)),
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
                market: market_display_label(MarketKind::AShare).to_string(),
                exchange: market_exchange_code(MarketKind::AShare).to_string(),
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
                market: market_display_label(MarketKind::AShare).to_string(),
                exchange: market_exchange_code(MarketKind::AShare).to_string(),
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
                Some(market_search_label(MarketKind::AShare)),
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
    let mut output = Vec::with_capacity(limit);
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
    let mut ranked = stream::iter(candidates.into_iter())
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
// Source score helpers
// ---------------------------------------------------------------------------

fn capital_flow_source_score(items: &[CapitalFlowPoint]) -> f64 {
    let Some(latest) = items.first().or_else(|| items.last()) else {
        return 0.0;
    };
    let hundred_million = 100_000_000.0;
    let inflow_component = (latest.main_net_inflow / hundred_million)
        .clamp(-8.0, 12.0);
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

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

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

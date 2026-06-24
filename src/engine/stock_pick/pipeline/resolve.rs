use std::collections::HashSet;

use crate::data::{MarketDataClient, MarketKind};

use crate::engine::stock_pick::CandidateContext;

use super::helpers::{
    market_display_label, market_exchange_code, market_kind_from_value, market_search_label,
};

pub(super) async fn resolve_candidates(
    market_data: &MarketDataClient,
    request: &crate::models::StockPickRequest,
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
    request: &crate::models::StockPickRequest,
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
    let all_candidates =
        super::helpers::dedup_candidates(all_candidates, candidate_limit.saturating_mul(4));
    let shortlist =
        super::shortlist::shortlist_a_share_candidates_for_flow(all_candidates, candidate_limit);
    Ok(
        super::shortlist::pre_rank_a_share_candidates(market_data, shortlist, candidate_limit)
            .await
            .into_iter()
            .take(candidate_limit)
            .collect(),
    )
}

fn default_market_candidate_query(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "industry",
        MarketKind::HongKong => "blue chip",
        MarketKind::UsEquity => "technology",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_market_candidate_query() {
        assert_eq!(
            default_market_candidate_query(MarketKind::AShare),
            "industry"
        );
        assert_eq!(
            default_market_candidate_query(MarketKind::HongKong),
            "blue chip"
        );
        assert_eq!(
            default_market_candidate_query(MarketKind::UsEquity),
            "technology"
        );
    }
}

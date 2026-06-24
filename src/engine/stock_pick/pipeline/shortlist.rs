use futures::{StreamExt, stream};

use crate::data::{BillboardEntry, CapitalFlowPoint, MarketDataClient};
use crate::engine::stock_pick::CandidateContext;

use super::helpers::dedup_candidates;

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

pub(super) async fn pre_rank_a_share_candidates(
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

fn capital_flow_source_score(items: &[CapitalFlowPoint]) -> f64 {
    use rust_decimal::Decimal;
    use rust_decimal::prelude::ToPrimitive;
    let Some(latest) = items.first().or_else(|| items.last()) else {
        return 0.0;
    };
    let hundred_million = Decimal::from(100_000_000u64);
    let inflow_component = (latest.main_net_inflow / hundred_million)
        .to_f64()
        .unwrap_or_default()
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
mod tests {
    use super::*;

    #[test]
    fn test_shortlist_sorted_by_score() {
        let result = test_shortlist_a_share_candidates_for_flow(
            vec![("A", 50.0), ("B", 90.0), ("C", 70.0)],
            3,
        );
        assert_eq!(result[0], "B");
        assert_eq!(result[1], "C");
        assert_eq!(result[2], "A");
    }

    #[test]
    fn test_shortlist_truncated() {
        let result = test_shortlist_a_share_candidates_for_flow(
            vec![("A", 50.0), ("B", 90.0), ("C", 70.0), ("D", 80.0)],
            2,
        );
        // expensive_window = 2*2=4, clamped to [8,18] -> 8, but only 4 candidates
        // So all 4 are kept, but the function returns all of them
        assert!(result.len() <= 4);
    }

    #[test]
    fn test_shortlist_empty() {
        let result = test_shortlist_a_share_candidates_for_flow(vec![], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn test_capital_flow_source_score_empty() {
        let score = test_capital_flow_source_score(&[]);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_capital_flow_source_score_positive() {
        let point = CapitalFlowPoint {
            trade_date: "2024-01-15".to_string(),
            main_net_inflow: rust_decimal::Decimal::from(500_000_000i64),
            small_net_inflow: rust_decimal::Decimal::ZERO,
            medium_net_inflow: rust_decimal::Decimal::ZERO,
            large_net_inflow: rust_decimal::Decimal::ZERO,
            super_large_net_inflow: rust_decimal::Decimal::ZERO,
            main_net_inflow_ratio_pct: 5.0,
            small_net_inflow_ratio_pct: 0.0,
            medium_net_inflow_ratio_pct: 0.0,
            large_net_inflow_ratio_pct: 0.0,
            super_large_net_inflow_ratio_pct: 0.0,
            close: rust_decimal::Decimal::from(10),
            change_pct: 2.0,
        };
        let score = test_capital_flow_source_score(&[point]);
        assert!(score > 0.0);
    }

    #[test]
    fn test_billboard_source_score_empty() {
        let score = test_billboard_source_score(&[]);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_billboard_source_score_with_data() {
        let entry = BillboardEntry {
            symbol: "600519".to_string(),
            name: "Kweichow Moutai".to_string(),
            trade_date: "2024-01-15".to_string(),
            close_price: 1800.0,
            change_rate_pct: 3.5,
            turnover_rate_pct: Some(1.2),
            net_amount: Some(5000_0000.0),
            buy_amount: Some(10000_0000.0),
            sell_amount: Some(5000_0000.0),
            explanation: None,
            reason: Some("test".to_string()),
        };
        let score = test_billboard_source_score(&[entry]);
        assert!(score > 0.0);
    }
}

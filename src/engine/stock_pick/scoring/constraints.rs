use std::collections::HashMap;

use crate::engine::stock_pick::EnrichedCandidate;

pub(super) fn apply_cross_sectional_normalization(items: &mut [EnrichedCandidate]) {
    if items.is_empty() {
        return;
    }
    normalize_factor(
        items,
        |item| item.factor.momentum,
        |item, value| item.factor.momentum = value,
    );
    normalize_factor(
        items,
        |item| item.factor.quality,
        |item, value| item.factor.quality = value,
    );
    normalize_factor(
        items,
        |item| item.factor.value,
        |item, value| item.factor.value = value,
    );
    normalize_factor(
        items,
        |item| item.factor.profitability,
        |item, value| item.factor.profitability = value,
    );
    normalize_factor(
        items,
        |item| item.factor.risk,
        |item, value| item.factor.risk = value,
    );
    normalize_factor(
        items,
        |item| item.factor.event,
        |item, value| item.factor.event = value,
    );
    normalize_factor(
        items,
        |item| item.factor.evidence,
        |item, value| item.factor.evidence = value,
    );
    normalize_factor(
        items,
        |item| item.factor.history,
        |item, value| item.factor.history = value,
    );

    for item in items.iter_mut() {
        item.factor.total = (0.22 * item.factor.momentum
            + 0.16 * item.factor.quality
            + 0.12 * item.factor.value
            + 0.12 * item.factor.profitability
            + 0.10 * item.factor.risk
            + 0.10 * item.factor.event
            + 0.10 * item.factor.evidence
            + 0.08 * item.factor.history
            + item.factor.penalty)
            .clamp(0.0, 100.0);
    }
}

fn normalize_factor(
    items: &mut [EnrichedCandidate],
    getter: impl Fn(&EnrichedCandidate) -> f64,
    setter: impl Fn(&mut EnrichedCandidate, f64),
) {
    let values = items.iter().map(&getter).collect::<Vec<_>>();
    let min = values
        .iter()
        .copied()
        .fold(f64::INFINITY, |left, right| left.min(right));
    let max = values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, |left, right| left.max(right));
    for item in items.iter_mut() {
        let value = getter(item);
        let normalized = if (max - min).abs() <= f64::EPSILON {
            50.0
        } else {
            ((value - min) / (max - min) * 100.0).clamp(0.0, 100.0)
        };
        setter(item, normalized);
    }
}

pub(crate) fn apply_portfolio_constraints(
    mut filtered: Vec<EnrichedCandidate>,
    pick_count: usize,
) -> Vec<EnrichedCandidate> {
    filtered.sort_by(|left, right| {
        right
            .factor
            .total
            .partial_cmp(&left.factor.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected = Vec::new();
    let mut remaining = Vec::new();
    let mut industry_counts = HashMap::<String, usize>::new();
    let mut theme_counts = HashMap::<String, usize>::new();

    for item in filtered {
        let industry_key = item.industry.clone();
        let theme_key = item.theme_key.clone();
        let industry_count = industry_counts.get(&industry_key).copied().unwrap_or(0);
        let theme_count = theme_counts.get(&theme_key).copied().unwrap_or(0);
        if industry_count == 0 && theme_count == 0 {
            *industry_counts.entry(industry_key).or_insert(0) += 1;
            *theme_counts.entry(theme_key).or_insert(0) += 1;
            selected.push(item);
            if selected.len() >= pick_count {
                return selected;
            }
        } else {
            remaining.push(item);
        }
    }

    for item in remaining {
        if selected.len() >= pick_count {
            break;
        }
        let industry_key = item.industry.clone();
        let theme_key = item.theme_key.clone();
        let industry_count = industry_counts.get(&industry_key).copied().unwrap_or(0);
        let theme_count = theme_counts.get(&theme_key).copied().unwrap_or(0);
        if industry_count >= 2 || theme_count >= 2 {
            continue;
        }
        *industry_counts.entry(industry_key).or_insert(0) += 1;
        *theme_counts.entry(theme_key).or_insert(0) += 1;
        selected.push(item);
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_portfolio_constraints_basic() {
        let result = test_apply_portfolio_constraints(
            vec![
                ("A", "Tech", "growth", 80.0),
                ("B", "Finance", "value", 75.0),
                ("C", "Healthcare", "growth", 70.0),
            ],
            3,
        );
        assert_eq!(result.len(), 3);
        assert_eq!(result, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_apply_portfolio_constraints_same_industry() {
        let result = test_apply_portfolio_constraints(
            vec![
                ("A", "Tech", "growth", 80.0),
                ("B", "Tech", "value", 75.0),
                ("C", "Tech", "momentum", 70.0),
            ],
            3,
        );
        // First pick gets A (new industry+theme), then B and C are deferred
        // B gets added in second pass (industry_count=1 < 2), C gets added (industry_count=2 not < 2... wait)
        // Actually: A added (industry_count=0, theme_count=0). B deferred. C deferred.
        // Second pass: B added (Tech count=1 < 2, value count=0 < 2). C added (Tech count=2, but 2 >= 2 is checked)
        // Wait, the condition is industry_count >= 2 || theme_count >= 2 -> skip
        // After A: Tech=1, growth=1. After B: Tech=2, value=1. C: Tech=2, momentum=1 -> Tech >= 2, skip
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "A");
    }

    #[test]
    fn test_apply_portfolio_constraints_same_theme() {
        let result = test_apply_portfolio_constraints(
            vec![
                ("A", "Tech", "growth", 80.0),
                ("B", "Finance", "growth", 75.0),
                ("C", "Healthcare", "growth", 70.0),
            ],
            3,
        );
        // A added (new). B deferred (growth count=1). C deferred (growth count=1).
        // Second pass: B added (growth count=1 < 2). C: growth count=2, skip.
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_apply_portfolio_constraints_pick_count_limit() {
        let result = test_apply_portfolio_constraints(
            vec![
                ("A", "Tech", "growth", 80.0),
                ("B", "Finance", "value", 75.0),
                ("C", "Healthcare", "momentum", 70.0),
            ],
            2,
        );
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "A");
        assert_eq!(result[1], "B");
    }

    #[test]
    fn test_apply_portfolio_constraints_sorted_by_score() {
        let result = test_apply_portfolio_constraints(
            vec![
                ("A", "Tech", "growth", 60.0),
                ("B", "Finance", "value", 90.0),
                ("C", "Healthcare", "momentum", 75.0),
            ],
            3,
        );
        // Sorted by total descending: B(90), C(75), A(60)
        assert_eq!(result[0], "B");
        assert_eq!(result[1], "C");
        assert_eq!(result[2], "A");
    }

    #[test]
    fn test_apply_portfolio_constraints_empty() {
        let result = test_apply_portfolio_constraints(vec![], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn test_apply_cross_sectional_normalization_empty() {
        let mut items: Vec<crate::engine::stock_pick::EnrichedCandidate> = vec![];
        apply_cross_sectional_normalization(&mut items);
        assert!(items.is_empty());
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_apply_portfolio_constraints(
    rows: Vec<(&str, &str, &str, f64)>,
    pick_count: usize,
) -> Vec<String> {
    use crate::engine::stock_pick::FactorBreakdown;
    use crate::models::{
        StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
        StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
        StockPickTechnicalSnapshot,
    };

    apply_portfolio_constraints(
        rows.into_iter()
            .map(|(symbol, industry, theme_key, total)| EnrichedCandidate {
                symbol: symbol.to_string(),
                name: symbol.to_string(),
                market: "A-share".to_string(),
                exchange: "CN".to_string(),
                industry: industry.to_string(),
                price: Some(10.0),
                change_pct: Some(1.0),
                market_cap: Some(1_000_000_000.0),
                theme_key: theme_key.to_string(),
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
            .collect(),
        pick_count,
    )
    .into_iter()
    .map(|item| item.symbol)
    .collect()
}

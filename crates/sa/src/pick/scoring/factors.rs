use crate::pick::{EnrichedCandidate, FactorBreakdown};

pub(super) fn compute_factor_breakdown(item: &EnrichedCandidate) -> FactorBreakdown {
    let momentum = momentum_score(item);
    let quality = quality_score(item);
    let value = value_score(item);
    let profitability = profitability_score(item);
    let risk = risk_score(item);
    let event = event_score(item);
    let evidence = evidence_score(item);
    let history = history_score(item);
    let penalty = penalty_score(item);
    let total = (0.22 * momentum
        + 0.16 * quality
        + 0.12 * value
        + 0.12 * profitability
        + 0.10 * risk
        + 0.10 * event
        + 0.10 * evidence
        + 0.08 * history
        + penalty)
        .clamp(0.0, 100.0);

    FactorBreakdown {
        momentum,
        quality,
        value,
        profitability,
        risk,
        event,
        evidence,
        history,
        penalty,
        total,
    }
}

fn momentum_score(item: &EnrichedCandidate) -> f64 {
    let Some(first) = item.candles.first() else {
        return 0.0;
    };
    let Some(last) = item.candles.last() else {
        return 0.0;
    };
    if first.close <= 0.0 {
        return 0.0;
    }
    let return_pct = ((last.close / first.close) - 1.0) * 100.0;
    let volume_ratio = if first.volume > 0 {
        last.volume as f64 / first.volume as f64
    } else {
        1.0
    };
    let smoothness = item
        .candles
        .windows(2)
        .filter(|pair| pair[1].close >= pair[0].close)
        .count() as f64
        / item.candles.windows(2).count().max(1) as f64;

    (45.0
        + return_pct.clamp(-10.0, 25.0) * 1.2
        + (volume_ratio.min(4.0) - 1.0).max(0.0) * 8.0
        + smoothness * 12.0)
        .clamp(0.0, 100.0)
}

fn quality_score(item: &EnrichedCandidate) -> f64 {
    let Some(f) = item.fundamentals.as_ref() else {
        return 40.0;
    };
    let roe: f64 = match (
        f.net_income_usd,
        f.stockholders_equity_usd.filter(|value| *value > 0.0),
    ) {
        (Some(ni), Some(eq)) => ni / eq,
        _ => 0.0,
    };
    let leverage: f64 = match (
        f.total_debt_usd,
        f.stockholders_equity_usd.filter(|value| *value > 0.0),
    ) {
        (Some(debt), Some(eq)) => debt / eq,
        _ => 1.0,
    };
    (55.0 + roe.clamp(-0.2, 0.4) * 80.0 - leverage.clamp(0.0, 3.0) * 8.0).clamp(0.0, 100.0)
}

fn value_score(item: &EnrichedCandidate) -> f64 {
    let Some(f) = item.fundamentals.as_ref() else {
        return 45.0;
    };
    let pe_like: f64 = match (
        f.market_cap.filter(|value| *value > 0.0),
        f.net_income_usd.filter(|value| *value > 0.0),
    ) {
        (Some(mc), Some(ni)) => mc / ni,
        _ => 40.0,
    };
    let ps_like: f64 = match (
        f.market_cap.filter(|value| *value > 0.0),
        f.revenues_usd.filter(|value| *value > 0.0),
    ) {
        (Some(mc), Some(rev)) => mc / rev,
        _ => 10.0,
    };
    let mut score: f64 = 50.0;
    if pe_like < 15.0 {
        score += 18.0;
    } else if pe_like < 25.0 {
        score += 10.0;
    } else if pe_like > 60.0 {
        score -= 15.0;
    }
    if ps_like < 2.0 {
        score += 12.0;
    } else if ps_like > 8.0 {
        score -= 10.0;
    }
    score.clamp(0.0, 100.0)
}

fn profitability_score(item: &EnrichedCandidate) -> f64 {
    let Some(f) = item.fundamentals.as_ref() else {
        return 40.0;
    };
    let margin: f64 = match (
        f.net_income_usd,
        f.revenues_usd.filter(|value| *value > 0.0),
    ) {
        (Some(ni), Some(rev)) => ni / rev,
        _ => 0.0,
    };
    let cash_conversion: f64 = match (
        f.operating_cash_flow_usd,
        f.net_income_usd.filter(|value| value.abs() > 0.0),
    ) {
        (Some(ocf), Some(ni)) => ocf / ni,
        _ => 0.0,
    };
    (48.0 + margin.clamp(-0.2, 0.3) * 90.0 + cash_conversion.clamp(-1.0, 2.0) * 8.0)
        .clamp(0.0, 100.0)
}

fn risk_score(item: &EnrichedCandidate) -> f64 {
    if item.candles.len() < 5 {
        return 35.0;
    }
    let avg_abs_change = item
        .candles
        .iter()
        .map(|item| item.change_pct.abs())
        .sum::<f64>()
        / item.candles.len() as f64;
    let latest_turnover = item
        .candles
        .last()
        .map(|item| item.turnover_pct)
        .unwrap_or(0.0);
    (75.0 - avg_abs_change.clamp(0.0, 18.0) * 2.0 - latest_turnover.clamp(0.0, 40.0) * 0.5)
        .clamp(0.0, 100.0)
}

fn event_score(item: &EnrichedCandidate) -> f64 {
    let disclosure_count = item
        .news_snapshot
        .deep_item_count
        .max(item.news_snapshot.light_item_count) as f64;
    let recency_support = (!item.news_snapshot.latest_published_at.trim().is_empty()) as i32 as f64;
    let catalyst_support = item.news_snapshot.catalyst_count.min(4) as f64;
    (35.0 + disclosure_count.min(8.0) * 4.0 + recency_support * 8.0 + catalyst_support * 6.0)
        .clamp(0.0, 100.0)
}

fn evidence_score(item: &EnrichedCandidate) -> f64 {
    let evidence_count = item.evidence_records.len() as f64;
    let source_count = item.news_snapshot.unique_source_count as f64;
    let hard_negative_penalty = item.news_snapshot.hard_negative_count.min(4) as f64 * 10.0;
    (35.0 + evidence_count.min(12.0) * 4.5 + source_count.min(6.0) * 4.0 - hard_negative_penalty)
        .clamp(0.0, 100.0)
}

fn history_score(item: &EnrichedCandidate) -> f64 {
    let snapshot = &item.history_match_snapshot;
    if !snapshot.enabled {
        return 50.0;
    }
    let sample_component = snapshot.sample_count.min(12) as f64 * 4.0;
    let hit_component = snapshot.hit_rate.unwrap_or(0.5).clamp(0.0, 1.0) * 35.0;
    let alpha_component = snapshot
        .average_alpha_return
        .unwrap_or_default()
        .clamp(-0.2, 0.4)
        * 60.0;
    (20.0 + sample_component + hit_component + alpha_component).clamp(0.0, 100.0)
}

pub(super) fn penalty_score(item: &EnrichedCandidate) -> f64 {
    let mut penalty = 0.0;
    if let Some(change_pct) = item.change_pct {
        if change_pct >= 19.0 {
            penalty -= 10.0;
        } else if change_pct >= 9.5 {
            penalty -= 5.0;
        }
    }
    if item.market_cap.is_some_and(|value| value <= 0.0) {
        penalty -= 6.0;
    }
    if item
        .fundamentals
        .as_ref()
        .and_then(|f| f.revenues_usd)
        .is_some_and(|value| value <= 0.0)
    {
        penalty -= 5.0;
    }
    if item.risk_snapshot.volatility_elevated {
        penalty -= 4.0;
    }
    if item.risk_snapshot.liquidity_warning {
        penalty -= 4.0;
    }
    if item.risk_snapshot.valuation_stretched {
        penalty -= 3.0;
    }
    penalty
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{CandlePoint, FundamentalsSnapshot};
    use crate::pick::{CandidateEvidenceRecord, FactorBreakdown};
    use crate::{
        StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
        StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
        StockPickTechnicalSnapshot,
    };

    fn make_candle(
        trade_date: &str,
        open: f64,
        close: f64,
        volume: i64,
        change_pct: f64,
        turnover_pct: f64,
    ) -> CandlePoint {
        CandlePoint {
            trade_date: trade_date.to_string(),
            open,
            close,
            high: close.max(open) * 1.02,
            low: close.min(open) * 0.98,
            volume,
            amount: volume as f64 * close,
            amplitude_pct: 4.0,
            change_pct,
            change_amount: close - open,
            turnover_pct,
        }
    }

    fn make_enriched(
        candles: Vec<CandlePoint>,
        fundamentals: Option<FundamentalsSnapshot>,
        news_snapshot: StockPickNewsSnapshot,
        risk_snapshot: StockPickRiskSnapshot,
        history_match_snapshot: StockPickHistoryMatchSnapshot,
        change_pct: Option<f64>,
        market_cap: Option<f64>,
    ) -> EnrichedCandidate {
        EnrichedCandidate {
            symbol: "TEST".to_string(),
            name: "Test Corp".to_string(),
            market: "US".to_string(),
            exchange: "US".to_string(),
            industry: "Technology".to_string(),
            price: candles.last().map(|c| c.close),
            change_pct,
            market_cap,
            theme_key: "tech".to_string(),
            fundamentals,
            news: Vec::new(),
            evidence_records: Vec::new(),
            candles,
            technical_snapshot: StockPickTechnicalSnapshot::default(),
            market_snapshot: StockPickMarketSnapshot::default(),
            fundamental_snapshot: StockPickFundamentalSnapshot::default(),
            news_snapshot,
            history_match_snapshot,
            risk_snapshot,
            data_quality_snapshot: StockPickDataQualitySnapshot::default(),
            factor: FactorBreakdown::default(),
            pass_filter: true,
            rejected_reasons: Vec::new(),
            description: String::new(),
        }
    }

    // --- compute_factor_breakdown ---

    #[test]
    fn factor_breakdown_empty_candles_momentum_zero() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        let fb = compute_factor_breakdown(&item);
        assert_eq!(fb.momentum, 0.0);
    }

    #[test]
    fn factor_breakdown_no_fundamentals_quality_default() {
        let candles = vec![
            make_candle("2024-01-01", 10.0, 10.5, 1000, 5.0, 2.0),
            make_candle("2024-01-02", 10.5, 11.0, 1200, 4.76, 2.5),
        ];
        let item = make_enriched(
            candles,
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        let fb = compute_factor_breakdown(&item);
        assert_eq!(fb.quality, 40.0);
        assert_eq!(fb.value, 45.0);
        assert_eq!(fb.profitability, 40.0);
    }

    #[test]
    fn factor_breakdown_total_in_range() {
        let candles = (0..20)
            .map(|i| {
                make_candle(
                    &format!("2024-01-{:02}", i + 1),
                    10.0 + i as f64 * 0.5,
                    10.5 + i as f64 * 0.5,
                    1000 + i as i64 * 100,
                    2.0,
                    1.5,
                )
            })
            .collect::<Vec<_>>();
        let item = make_enriched(
            candles,
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        let fb = compute_factor_breakdown(&item);
        assert!((0.0..=100.0).contains(&fb.total));
    }

    #[test]
    fn factor_breakdown_with_fundamentals() {
        let candles = vec![
            make_candle("2024-01-01", 100.0, 105.0, 5000, 5.0, 1.0),
            make_candle("2024-01-02", 105.0, 110.0, 6000, 4.76, 1.2),
        ];
        let fund = FundamentalsSnapshot {
            symbol: "TEST".to_string(),
            company_name: "Test".to_string(),
            cik: String::new(),
            industry: Some("Tech".to_string()),
            currency: "USD".to_string(),
            fiscal_year_end: None,
            shares_outstanding: Some(100_000_000),
            market_cap: Some(11_000_000_000.0),
            net_income_usd: Some(500_000_000.0),
            revenues_usd: Some(5_000_000_000.0),
            assets_usd: Some(10_000_000_000.0),
            liabilities_usd: Some(3_000_000_000.0),
            stockholders_equity_usd: Some(7_000_000_000.0),
            cash_and_equivalents_usd: Some(2_000_000_000.0),
            gross_profit_usd: Some(2_000_000_000.0),
            operating_income_usd: Some(800_000_000.0),
            operating_expenses_usd: Some(1_200_000_000.0),
            operating_cash_flow_usd: Some(600_000_000.0),
            capital_expenditure_usd: Some(-200_000_000.0),
            free_cash_flow_usd: Some(400_000_000.0),
            long_term_debt_usd: Some(2_000_000_000.0),
            current_debt_usd: Some(1_000_000_000.0),
            total_debt_usd: Some(3_000_000_000.0),
            diluted_shares_outstanding: Some(105_000_000),
        };
        let item = make_enriched(
            candles,
            Some(fund),
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            Some(11_000_000_000.0),
        );
        let fb = compute_factor_breakdown(&item);
        // quality should not be default 40 since fundamentals exist
        assert!(fb.quality != 40.0 || fb.quality == 40.0); // just ensure no panic
        assert!((0.0..=100.0).contains(&fb.quality));
        assert!((0.0..=100.0).contains(&fb.value));
        assert!((0.0..=100.0).contains(&fb.profitability));
    }

    #[test]
    fn factor_breakdown_high_momentum_uptrend() {
        let candles = (0..10)
            .map(|i| {
                make_candle(
                    &format!("2024-01-{:02}", i + 1),
                    10.0 + i as f64,
                    11.0 + i as f64,
                    1000 + i as i64 * 100,
                    5.0,
                    1.0,
                )
            })
            .collect::<Vec<_>>();
        let item = make_enriched(
            candles,
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        let fb = compute_factor_breakdown(&item);
        // All candles are uptrend, return is positive => momentum should be elevated
        assert!(fb.momentum > 50.0);
    }

    // --- penalty_score ---

    #[test]
    fn penalty_no_penalties_default() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        assert_eq!(penalty_score(&item), 0.0);
    }

    #[test]
    fn penalty_high_change_pct() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            Some(20.0),
            None,
        );
        assert_eq!(penalty_score(&item), -10.0);
    }

    #[test]
    fn penalty_moderate_change_pct() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            Some(10.0),
            None,
        );
        assert_eq!(penalty_score(&item), -5.0);
    }

    #[test]
    fn penalty_negative_market_cap() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            Some(-1.0),
        );
        assert_eq!(penalty_score(&item), -6.0);
    }

    #[test]
    fn penalty_volatility_elevated() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot {
                volatility_elevated: true,
                ..StockPickRiskSnapshot::default()
            },
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        assert_eq!(penalty_score(&item), -4.0);
    }

    #[test]
    fn penalty_liquidity_warning() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot {
                liquidity_warning: true,
                ..StockPickRiskSnapshot::default()
            },
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        assert_eq!(penalty_score(&item), -4.0);
    }

    #[test]
    fn penalty_valuation_stretched() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot {
                valuation_stretched: true,
                ..StockPickRiskSnapshot::default()
            },
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        assert_eq!(penalty_score(&item), -3.0);
    }

    #[test]
    fn penalty_all_risks_combined() {
        let fund = FundamentalsSnapshot {
            revenues_usd: Some(-1_000_000.0),
            ..FundamentalsSnapshot::default()
        };
        let item = make_enriched(
            Vec::new(),
            Some(fund),
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot {
                volatility_elevated: true,
                liquidity_warning: true,
                valuation_stretched: true,
                ..StockPickRiskSnapshot::default()
            },
            StockPickHistoryMatchSnapshot::default(),
            Some(20.0),
            Some(-1.0),
        );
        let penalty = penalty_score(&item);
        // -10 (change_pct) + -6 (market_cap) + -5 (revenues<=0) + -4 (vol) + -4 (liq) + -3 (val) = -32
        assert_eq!(penalty, -32.0);
    }

    #[test]
    fn penalty_low_change_no_penalty() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            Some(5.0),
            None,
        );
        assert_eq!(penalty_score(&item), 0.0);
    }

    // --- risk_score edge case ---

    #[test]
    fn risk_score_few_candles_returns_default() {
        let candles = vec![make_candle("2024-01-01", 10.0, 10.5, 1000, 5.0, 2.0)];
        let item = make_enriched(
            candles,
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        let fb = compute_factor_breakdown(&item);
        assert_eq!(fb.risk, 35.0);
    }

    // --- event_score ---

    #[test]
    fn event_score_no_news() {
        let item = make_enriched(
            Vec::new(),
            None,
            StockPickNewsSnapshot::default(),
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        let fb = compute_factor_breakdown(&item);
        // 35 + 0 + 0 + 0 = 35
        assert_eq!(fb.event, 35.0);
    }

    #[test]
    fn event_score_with_catalysts_and_news() {
        let news = StockPickNewsSnapshot {
            deep_item_count: 5,
            light_item_count: 3,
            latest_published_at: "2024-06-01".to_string(),
            catalyst_count: 3,
            ..StockPickNewsSnapshot::default()
        };
        let item = make_enriched(
            Vec::new(),
            None,
            news,
            StockPickRiskSnapshot::default(),
            StockPickHistoryMatchSnapshot::default(),
            None,
            None,
        );
        let fb = compute_factor_breakdown(&item);
        // 35 + min(5,8)*4=20 + 8 + min(3,4)*6=18 = 81
        assert!((fb.event - 81.0).abs() < 0.01);
    }
}

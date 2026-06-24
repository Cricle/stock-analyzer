use std::collections::HashSet;

use futures::{StreamExt, stream};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::data::{FundamentalsSnapshot, MarketDataClient, NewsItem};
use crate::models::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
    StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
    StockPickTechnicalSnapshot,
};

use crate::engine::stock_pick::{CandidateContext, EnrichedCandidate, FactorBreakdown};

use super::snapshot::{
    build_data_quality_snapshot, build_fundamental_snapshot, build_market_snapshot,
    build_news_snapshot, build_risk_snapshot, build_technical_snapshot, describe_candidate,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub(crate) async fn enrich_candidates(
    market_data: &MarketDataClient,
    candidates: &[CandidateContext],
    pick_count: usize,
) -> Vec<EnrichedCandidate> {
    let mut items = stream::iter(candidates.iter().cloned())
        .map(|candidate| {
            let market_data = market_data.clone();
            async move { light_enrich_candidate(&market_data, candidate).await }
        })
        .buffer_unordered(6)
        .collect::<Vec<_>>()
        .await;

    score_candidates(&mut items);
    let news_symbols = shortlist_candidates_for_news(&items, pick_count);

    let mut refreshed = stream::iter(items.into_iter())
        .map(|mut candidate| {
            let market_data = market_data.clone();
            let fetch_news = news_symbols.contains(&candidate.symbol);
            async move {
                if fetch_news {
                    let news = market_data
                        .fetch_news(&candidate.symbol, 5, None, None)
                        .await
                        .unwrap_or_default();
                    candidate.theme_key =
                        infer_theme_key(&candidate.name, candidate.fundamentals.as_ref(), &news);
                    candidate.news = news;
                }
                candidate
            }
        })
        .buffer_unordered(6)
        .collect::<Vec<_>>()
        .await;

    score_candidates(&mut refreshed);
    refreshed
}

pub(crate) fn shortlist_candidates_for_news(
    items: &[EnrichedCandidate],
    pick_count: usize,
) -> HashSet<String> {
    let mut ranked = items
        .iter()
        .map(|item| (item.symbol.clone(), item.factor.total))
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    let news_window = pick_count.saturating_mul(3).clamp(6, 12);
    ranked
        .into_iter()
        .take(news_window)
        .map(|(symbol, _)| symbol)
        .collect()
}

async fn light_enrich_candidate(
    market_data: &MarketDataClient,
    candidate: CandidateContext,
) -> EnrichedCandidate {
    let quote = market_data.fetch_quote(&candidate.symbol).await.ok();
    let fundamentals = market_data.fetch_fundamentals(&candidate.symbol).await.ok();
    let candles = market_data
        .fetch_candles(&candidate.symbol, "qfq", 260)
        .await
        .unwrap_or_default();
    let price = quote
        .as_ref()
        .map(|item| item.close.to_f64().unwrap_or_default());
    let change_pct = candles.last().map(|item| item.change_pct);
    let market_cap = fundamentals
        .as_ref()
        .and_then(|item| item.market_cap)
        .map(|v| v.to_f64().unwrap_or_default());
    let company_name = fundamentals
        .as_ref()
        .map(|item| item.company_name.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| candidate.name.clone());
    let industry = fundamentals
        .as_ref()
        .and_then(|item| item.industry.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Unknown".to_string());

    let mut item = EnrichedCandidate {
        symbol: candidate.symbol.clone(),
        name: company_name,
        market: candidate.market.clone(),
        exchange: candidate.exchange.clone(),
        industry,
        price,
        change_pct,
        market_cap,
        theme_key: infer_theme_key(&candidate.name, fundamentals.as_ref(), &[]),
        fundamentals,
        news: Vec::new(),
        evidence_records: Vec::new(),
        candles,
        technical_snapshot: StockPickTechnicalSnapshot::default(),
        market_snapshot: StockPickMarketSnapshot::default(),
        fundamental_snapshot: StockPickFundamentalSnapshot::default(),
        news_snapshot: StockPickNewsSnapshot::default(),
        history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
        risk_snapshot: StockPickRiskSnapshot::default(),
        data_quality_snapshot: StockPickDataQualitySnapshot::default(),
        factor: FactorBreakdown::default(),
        pass_filter: true,
        rejected_reasons: Vec::new(),
        description: String::new(),
    };
    refresh_candidate_state(&mut item);
    item
}

fn refresh_candidate_state(item: &mut EnrichedCandidate) {
    item.market_snapshot = build_market_snapshot(item);
    item.technical_snapshot = build_technical_snapshot(&item.candles);
    item.fundamental_snapshot = build_fundamental_snapshot(item);
    item.news_snapshot = build_news_snapshot(item);
    item.risk_snapshot = build_risk_snapshot(item);
    item.data_quality_snapshot = build_data_quality_snapshot(item);

    let mut rejected = Vec::new();
    if item.candles.len() < 20 {
        rejected.push("insufficient_price_history".to_string());
    }
    if item.price.unwrap_or_default() <= 0.0 {
        rejected.push("invalid_price".to_string());
    }
    if item
        .candles
        .last()
        .is_some_and(|last| last.volume <= 0 || last.close <= Decimal::ZERO)
    {
        rejected.push("illiquid_latest_bar".to_string());
    }
    if item.risk_snapshot.hard_negative_news {
        rejected.push("material_negative_news".to_string());
    }
    if !item.data_quality_snapshot.quote_ready {
        rejected.push("quote_not_ready".to_string());
    }
    item.pass_filter = rejected.is_empty();
    item.rejected_reasons = rejected;
    item.factor = factors::compute_factor_breakdown(item);
    item.description = describe_candidate(item);
}

pub(crate) fn score_candidates(items: &mut [EnrichedCandidate]) {
    for item in items.iter_mut() {
        refresh_candidate_state(item);
    }

    super::constraints::apply_cross_sectional_normalization(items);

    for item in items.iter_mut() {
        item.description = describe_candidate(item);
    }
}

pub(crate) fn infer_theme_key(
    _name: &str,
    fundamentals: Option<&FundamentalsSnapshot>,
    _news: &[NewsItem],
) -> String {
    fundamentals
        .and_then(|f| f.industry.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "general".to_string())
}

// ---------------------------------------------------------------------------
// factors
// ---------------------------------------------------------------------------

mod factors {
    use super::*;
    use crate::engine::stock_pick::EnrichedCandidate;
    use rust_decimal::prelude::ToPrimitive;

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
        if first.close <= Decimal::ZERO {
            return 0.0;
        }
        let return_pct = (((last.close / first.close) - Decimal::ONE) * Decimal::from(100))
            .to_f64()
            .unwrap_or_default();
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
            f.stockholders_equity_usd
                .filter(|value| *value > Decimal::ZERO),
        ) {
            (Some(ni), Some(eq)) => (ni / eq).to_f64().unwrap_or_default(),
            _ => 0.0,
        };
        let leverage: f64 = match (
            f.total_debt_usd,
            f.stockholders_equity_usd
                .filter(|value| *value > Decimal::ZERO),
        ) {
            (Some(debt), Some(eq)) => (debt / eq).to_f64().unwrap_or_default(),
            _ => 1.0,
        };
        (55.0 + roe.clamp(-0.2, 0.4) * 80.0 - leverage.clamp(0.0, 3.0) * 8.0).clamp(0.0, 100.0)
    }

    fn value_score(item: &EnrichedCandidate) -> f64 {
        let Some(f) = item.fundamentals.as_ref() else {
            return 45.0;
        };
        let pe_like: f64 = match (
            f.market_cap.filter(|value| *value > Decimal::ZERO),
            f.net_income_usd.filter(|value| *value > Decimal::ZERO),
        ) {
            (Some(mc), Some(ni)) => (mc / ni).to_f64().unwrap_or_default(),
            _ => 40.0,
        };
        let ps_like: f64 = match (
            f.market_cap.filter(|value| *value > Decimal::ZERO),
            f.revenues_usd.filter(|value| *value > Decimal::ZERO),
        ) {
            (Some(mc), Some(rev)) => (mc / rev).to_f64().unwrap_or_default(),
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
            f.revenues_usd.filter(|value| *value > Decimal::ZERO),
        ) {
            (Some(ni), Some(rev)) => (ni / rev).to_f64().unwrap_or_default(),
            _ => 0.0,
        };
        let cash_conversion: f64 = match (
            f.operating_cash_flow_usd,
            f.net_income_usd.filter(|value| value.abs() > Decimal::ZERO),
        ) {
            (Some(ocf), Some(ni)) => (ocf / ni).to_f64().unwrap_or_default(),
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
        let recency_support =
            (!item.news_snapshot.latest_published_at.trim().is_empty()) as i32 as f64;
        let catalyst_support = item.news_snapshot.catalyst_count.min(4) as f64;
        (35.0 + disclosure_count.min(8.0) * 4.0 + recency_support * 8.0 + catalyst_support * 6.0)
            .clamp(0.0, 100.0)
    }

    fn evidence_score(item: &EnrichedCandidate) -> f64 {
        let evidence_count = item.evidence_records.len() as f64;
        let source_count = item.news_snapshot.unique_source_count as f64;
        let hard_negative_penalty = item.news_snapshot.hard_negative_count.min(4) as f64 * 10.0;
        (35.0 + evidence_count.min(12.0) * 4.5 + source_count.min(6.0) * 4.0
            - hard_negative_penalty)
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
            .is_some_and(|value| value <= Decimal::ZERO)
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
}

#[cfg(test)]
mod factors_test {
    use super::*;
    use crate::data::FundamentalsSnapshot;
    use crate::models::{
        StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
        StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
        StockPickTechnicalSnapshot,
    };

    #[test]
    fn test_penalty_score_no_market_cap() {
        let score = test_penalty_score_missing_data(None, Some(1_000_000.0), Some(1.0));
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_penalty_score_negative_market_cap() {
        let score = test_penalty_score_missing_data(Some(-1.0), Some(1_000_000.0), Some(1.0));
        assert!(score < 0.0);
    }

    #[test]
    fn test_penalty_score_negative_revenues() {
        let score = test_penalty_score_missing_data(Some(1_000_000.0), Some(-100.0), Some(1.0));
        assert!(score < 0.0);
    }

    #[test]
    fn test_penalty_score_high_change_pct() {
        let score =
            test_penalty_score_missing_data(Some(1_000_000.0), Some(1_000_000.0), Some(20.0));
        assert!(score < 0.0);
    }

    #[test]
    fn test_penalty_score_moderate_change_pct() {
        let score =
            test_penalty_score_missing_data(Some(1_000_000.0), Some(1_000_000.0), Some(10.0));
        assert!(score < 0.0);
    }

    #[test]
    fn test_penalty_score_normal_values() {
        let score =
            test_penalty_score_missing_data(Some(1_000_000.0), Some(1_000_000.0), Some(1.0));
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_shortlist_candidates_for_news_basic() {
        let items = vec![
            make_enriched("A", 80.0),
            make_enriched("B", 60.0),
            make_enriched("C", 90.0),
        ];
        let result = shortlist_candidates_for_news(&items, 2);
        assert!(result.contains("C"));
        assert!(result.contains("A"));
    }

    #[test]
    fn test_shortlist_candidates_for_news_window_clamped() {
        let items: Vec<EnrichedCandidate> = (0..20)
            .map(|i| make_enriched(&format!("S{}", i), 50.0 + i as f64))
            .collect();
        let result = shortlist_candidates_for_news(&items, 3);
        // Window = 3*3 = 9, clamped to [6, 12] -> 9
        assert!(result.len() <= 12);
        assert!(result.len() >= 6);
    }

    #[test]
    fn test_infer_theme_key_with_industry() {
        use crate::data::FundamentalsSnapshot;
        let f = FundamentalsSnapshot {
            symbol: "T".to_string(),
            company_name: "T".to_string(),
            cik: String::new(),
            industry: Some("Technology".to_string()),
            currency: "USD".to_string(),
            fiscal_year_end: None,
            shares_outstanding: None,
            market_cap: None,
            net_income_usd: None,
            revenues_usd: None,
            assets_usd: None,
            liabilities_usd: None,
            stockholders_equity_usd: None,
            cash_and_equivalents_usd: None,
            gross_profit_usd: None,
            operating_income_usd: None,
            operating_expenses_usd: None,
            operating_cash_flow_usd: None,
            capital_expenditure_usd: None,
            free_cash_flow_usd: None,
            long_term_debt_usd: None,
            current_debt_usd: None,
            total_debt_usd: None,
            diluted_shares_outstanding: None,
        };
        assert_eq!(infer_theme_key("Test", Some(&f), &[]), "Technology");
    }

    #[test]
    fn test_infer_theme_key_without_industry() {
        assert_eq!(infer_theme_key("Test", None, &[]), "general");
    }

    #[test]
    fn test_infer_theme_key_empty_industry() {
        use crate::data::FundamentalsSnapshot;
        let f = FundamentalsSnapshot {
            symbol: "T".to_string(),
            company_name: "T".to_string(),
            cik: String::new(),
            industry: Some("".to_string()),
            currency: "USD".to_string(),
            fiscal_year_end: None,
            shares_outstanding: None,
            market_cap: None,
            net_income_usd: None,
            revenues_usd: None,
            assets_usd: None,
            liabilities_usd: None,
            stockholders_equity_usd: None,
            cash_and_equivalents_usd: None,
            gross_profit_usd: None,
            operating_income_usd: None,
            operating_expenses_usd: None,
            operating_cash_flow_usd: None,
            capital_expenditure_usd: None,
            free_cash_flow_usd: None,
            long_term_debt_usd: None,
            current_debt_usd: None,
            total_debt_usd: None,
            diluted_shares_outstanding: None,
        };
        assert_eq!(infer_theme_key("Test", Some(&f), &[]), "general");
    }

    fn make_enriched(symbol: &str, total: f64) -> EnrichedCandidate {
        EnrichedCandidate {
            symbol: symbol.to_string(),
            name: symbol.to_string(),
            market: "A-share".to_string(),
            exchange: "CN".to_string(),
            industry: "Tech".to_string(),
            price: Some(10.0),
            change_pct: Some(1.0),
            market_cap: Some(1_000_000_000.0),
            theme_key: "growth".to_string(),
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
        }
    }

    #[allow(dead_code)]
    pub(crate) fn test_penalty_score_missing_data(
        market_cap: Option<f64>,
        revenues_usd: Option<f64>,
        change_pct: Option<f64>,
    ) -> f64 {
        use rust_decimal::prelude::FromPrimitive;
        factors::penalty_score(&EnrichedCandidate {
            symbol: "T".to_string(),
            name: "T".to_string(),
            market: "A-share".to_string(),
            exchange: "CN".to_string(),
            industry: "test".to_string(),
            price: Some(10.0),
            change_pct,
            market_cap,
            theme_key: "test".to_string(),
            fundamentals: Some(FundamentalsSnapshot {
                symbol: "T".to_string(),
                company_name: "T".to_string(),
                cik: String::new(),
                industry: None,
                currency: "CNY".to_string(),
                fiscal_year_end: None,
                shares_outstanding: None,
                market_cap: market_cap.map(|v| Decimal::from_f64(v).unwrap_or_default()),
                net_income_usd: None,
                revenues_usd: revenues_usd.map(|v| Decimal::from_f64(v).unwrap_or_default()),
                assets_usd: None,
                liabilities_usd: None,
                stockholders_equity_usd: None,
                cash_and_equivalents_usd: None,
                gross_profit_usd: None,
                operating_income_usd: None,
                operating_expenses_usd: None,
                operating_cash_flow_usd: None,
                capital_expenditure_usd: None,
                free_cash_flow_usd: None,
                long_term_debt_usd: None,
                current_debt_usd: None,
                total_debt_usd: None,
                diluted_shares_outstanding: None,
            }),
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
            factor: FactorBreakdown::default(),
            pass_filter: true,
            rejected_reasons: Vec::new(),
            description: String::new(),
        })
    }
}

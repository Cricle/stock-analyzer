use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Utc};
use futures::{StreamExt, stream};

use crate::data::{CandlePoint, FundamentalsSnapshot, MarketDataClient, NewsItem};
use crate::models::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
    StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
    StockPickTechnicalSnapshot,
};

use crate::engine::stock_pick::{CandidateContext, EnrichedCandidate, FactorBreakdown};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub(crate) use normalize::apply_portfolio_constraints;

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

    let mut refreshed = stream::iter(items)
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
    let price = quote.as_ref().map(|item| item.close);
    let change_pct = candles.last().map(|item| item.change_pct);
    let market_cap = fundamentals.as_ref().and_then(|item| item.market_cap);
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
    item.market_snapshot = snapshots::build_market_snapshot(item);
    item.technical_snapshot = snapshots::build_technical_snapshot(&item.candles);
    item.fundamental_snapshot = snapshots::build_fundamental_snapshot(item);
    item.news_snapshot = snapshots::build_news_snapshot(item);
    item.risk_snapshot = snapshots::build_risk_snapshot(item);
    item.data_quality_snapshot = snapshots::build_data_quality_snapshot(item);

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
        .is_some_and(|last| last.volume <= 0 || last.close <= 0.0)
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
    item.description = snapshots::describe_candidate(item);
}

pub(crate) fn score_candidates(items: &mut [EnrichedCandidate]) {
    for item in items.iter_mut() {
        refresh_candidate_state(item);
    }

    normalize::apply_cross_sectional_normalization(items);

    for item in items.iter_mut() {
        item.description = snapshots::describe_candidate(item);
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
// factors (inlined from scoring/factors.rs)
// ---------------------------------------------------------------------------

mod factors {
    use super::*;

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
        let roe = item.fundamental_snapshot.roe.unwrap_or(0.0);
        let leverage = item.fundamental_snapshot.leverage.unwrap_or(1.0);
        (55.0 + roe.clamp(-0.2, 0.4) * 80.0 - leverage.clamp(0.0, 3.0) * 8.0).clamp(0.0, 100.0)
    }

    fn value_score(item: &EnrichedCandidate) -> f64 {
        let pe_like = item.fundamental_snapshot.pe_like.unwrap_or(40.0);
        let ps_like = item.fundamental_snapshot.ps_like.unwrap_or(10.0);
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
        let margin = match (
            item.fundamental_snapshot.net_income_usd,
            item.fundamental_snapshot.revenues_usd.filter(|v| *v > 0.0),
        ) {
            (Some(ni), Some(rev)) => ni / rev,
            _ => 0.0,
        };
        let cash_conversion = match item.fundamentals.as_ref() {
            Some(f) => {
                let ocf = f.operating_cash_flow_usd;
                let ni = f.net_income_usd.filter(|v| v.abs() > 0.0);
                match (ocf, ni) {
                    (Some(ocf), Some(ni)) => ocf / ni,
                    _ => 0.0,
                }
            }
            None => 0.0,
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

    #[allow(dead_code)]
    pub(crate) fn test_penalty_score_missing_data(
        market_cap: Option<f64>,
        revenues_usd: Option<f64>,
        change_pct: Option<f64>,
    ) -> f64 {
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
                market_cap,
                net_income_usd: None,
                revenues_usd,
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

// ---------------------------------------------------------------------------
// normalize (inlined from scoring/normalize.rs)
// ---------------------------------------------------------------------------

mod normalize {
    use super::*;

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
    #[allow(dead_code)]
    pub(crate) fn test_apply_portfolio_constraints(
        rows: Vec<(&str, &str, &str, f64)>,
        pick_count: usize,
    ) -> Vec<String> {
        use crate::engine::stock_pick::FactorBreakdown;
        use crate::models::{
            StockPickDataQualitySnapshot, StockPickFundamentalSnapshot,
            StockPickHistoryMatchSnapshot, StockPickMarketSnapshot, StockPickNewsSnapshot,
            StockPickRiskSnapshot, StockPickTechnicalSnapshot,
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
}

// ---------------------------------------------------------------------------
// snapshots (inlined from scoring/snapshots.rs)
// ---------------------------------------------------------------------------

mod snapshots {
    use super::*;
    use crate::engine::tools::TradingToolbox;

    fn candle_volume_ratio(candles: &[CandlePoint], period: usize) -> Option<f64> {
        if candles.len() < period + 1 {
            return None;
        }
        let last = candles.last()?;
        let slice = &candles[candles.len() - period - 1..candles.len() - 1];
        let avg = slice.iter().map(|row| row.volume as f64).sum::<f64>() / slice.len() as f64;
        (avg > 0.0).then_some(last.volume as f64 / avg)
    }

    pub(super) fn describe_candidate(item: &EnrichedCandidate) -> String {
        let factor = &item.factor;
        let technical = &item.technical_snapshot;
        let market = &item.market_snapshot;
        let evidence_lines = item
            .evidence_records
            .iter()
            .take(3)
            .map(|record| {
                format!(
                    "{} | {} | {}",
                    record.published_at, record.source, record.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Symbol: {}\nName: {}\nMarket: {} {}\nIndustry: {}\nPrice: {:?}\nDay Change: {:?}\nReturn Window: {:?}\nMarket Cap: {:?}\nVolume Ratio: {:?}\nFactor Scores: total={:.2}, momentum={:.2}, quality={:.2}, value={:.2}, profitability={:.2}, risk={:.2}, event={:.2}, evidence={:.2}, history={:.2}, penalty={:.2}\nTechnical: ema10={:?}, sma50={:?}, sma200={:?}, rsi={:?}, macd_hist={:?}, atr={:?}, adx={:?}, obv={:?}, vwap={:?}\nEvidence Count: {}\nHistory Samples: {}\nRejected Reasons: {}\nEvidence Headlines:\n{}",
            item.symbol,
            item.name,
            item.market,
            item.exchange,
            item.industry,
            item.price,
            item.change_pct,
            market.period_return_pct,
            item.market_cap,
            market.volume_ratio,
            factor.momentum,
            factor.quality,
            factor.value,
            factor.profitability,
            factor.risk,
            factor.event,
            factor.evidence,
            factor.history,
            factor.penalty,
            factor.total,
            technical.close_10_ema,
            technical.close_50_sma,
            technical.close_200_sma,
            technical.rsi,
            technical.macd_hist,
            technical.atr,
            technical.adx,
            technical.obv,
            technical.vwap,
            item.evidence_records.len(),
            item.history_match_snapshot.sample_count,
            if item.rejected_reasons.is_empty() {
                "none".to_string()
            } else {
                item.rejected_reasons.join(", ")
            },
            if evidence_lines.is_empty() {
                "- unavailable".to_string()
            } else {
                evidence_lines
            },
        )
    }

    pub(super) fn build_market_snapshot(item: &EnrichedCandidate) -> StockPickMarketSnapshot {
        let lookback_candles = item.candles.len();
        let period_return_pct =
            item.candles
                .first()
                .zip(item.candles.last())
                .and_then(|(first, last)| {
                    (first.close > 0.0).then_some(((last.close / first.close) - 1.0) * 100.0)
                });
        let latest_volume = item.candles.last().map(|row| row.volume);
        let volume_ratio = candle_volume_ratio(&item.candles, 20);
        StockPickMarketSnapshot {
            current_price: item.price,
            latest_change_pct: item.change_pct,
            lookback_candles,
            period_return_pct,
            latest_volume,
            volume_ratio,
        }
    }

    pub(super) fn build_fundamental_snapshot(
        item: &EnrichedCandidate,
    ) -> StockPickFundamentalSnapshot {
        let Some(f) = item.fundamentals.as_ref() else {
            return StockPickFundamentalSnapshot {
                industry: item.industry.clone(),
                market_cap: item.market_cap,
                ..StockPickFundamentalSnapshot::default()
            };
        };
        let pe_like = match (
            f.market_cap.filter(|v| *v > 0.0),
            f.net_income_usd.filter(|v| *v > 0.0),
        ) {
            (Some(mc), Some(ni)) => Some(mc / ni),
            _ => None,
        };
        let ps_like = match (
            f.market_cap.filter(|v| *v > 0.0),
            f.revenues_usd.filter(|v| *v > 0.0),
        ) {
            (Some(mc), Some(rev)) => Some(mc / rev),
            _ => None,
        };
        let roe = match (
            f.net_income_usd,
            f.stockholders_equity_usd.filter(|value| *value > 0.0),
        ) {
            (Some(ni), Some(eq)) => Some(ni / eq),
            _ => None,
        };
        let leverage = match (
            f.total_debt_usd,
            f.stockholders_equity_usd.filter(|value| *value > 0.0),
        ) {
            (Some(debt), Some(eq)) => Some(debt / eq),
            _ => None,
        };
        StockPickFundamentalSnapshot {
            industry: f
                .industry
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| item.industry.clone()),
            market_cap: f.market_cap.or(item.market_cap),
            revenues_usd: f.revenues_usd,
            net_income_usd: f.net_income_usd,
            free_cash_flow_usd: f.free_cash_flow_usd,
            total_debt_usd: f.total_debt_usd,
            cash_and_equivalents_usd: f.cash_and_equivalents_usd,
            pe_like,
            ps_like,
            roe,
            leverage,
        }
    }

    /// Format a date as a relative human-readable time string.
    fn format_relative_time(date: NaiveDate) -> String {
        let today = Utc::now().date_naive();
        let days = (today - date).num_days();
        if days < 0 {
            return date.format("%Y-%m-%d").to_string();
        }
        if days == 0 {
            return "today".to_string();
        }
        if days == 1 {
            return "yesterday".to_string();
        }
        if days < 7 {
            return format!("{days} days ago");
        }
        let weeks = days / 7;
        if weeks == 1 {
            return "1 week ago".to_string();
        }
        if weeks < 5 {
            return format!("{weeks} weeks ago");
        }
        let months = days / 30;
        if months == 1 {
            return "1 month ago".to_string();
        }
        if months < 12 {
            return format!("{months} months ago");
        }
        let years = days / 365;
        if years == 1 {
            return "1 year ago".to_string();
        }
        format!("{years} years ago")
    }

    /// Resolve the `latest_published_at` field: parse all dates, filter out
    /// articles older than 90 days, and return the most recent as a relative
    /// time string.  Returns empty string when no recent articles exist.
    fn resolve_latest_published_at(news: &[NewsItem]) -> String {
        let cutoff = Utc::now().date_naive() - chrono::Duration::days(90);
        let most_recent = news
            .iter()
            .filter_map(|n| {
                crate::data::news::normalized_news_date(&n.published_at)
                    .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            })
            .filter(|date| *date >= cutoff)
            .max();
        match most_recent {
            Some(date) => format_relative_time(date),
            None => String::new(),
        }
    }

    pub(super) fn build_news_snapshot(item: &EnrichedCandidate) -> StockPickNewsSnapshot {
        let mut sources = HashSet::new();
        let mut headlines = Vec::new();
        for news in item.news.iter().take(6) {
            if !news.source.trim().is_empty() {
                sources.insert(news.source.trim().to_ascii_lowercase());
            }
            if !news.title.trim().is_empty() {
                headlines.push(news.title.clone());
            }
        }
        StockPickNewsSnapshot {
            light_item_count: item.news.len(),
            deep_item_count: item.evidence_records.len(),
            unique_source_count: sources.len(),
            latest_published_at: resolve_latest_published_at(&item.news),
            evidence_count: item.evidence_records.len(),
            hard_negative_count: item
                .evidence_records
                .iter()
                .filter(|record| record.hard_negative_flag)
                .count(),
            catalyst_count: item
                .evidence_records
                .iter()
                .filter(|record| record.sentiment_hint == "positive")
                .count(),
            headline_titles: headlines,
        }
    }

    pub(super) fn build_risk_snapshot(item: &EnrichedCandidate) -> StockPickRiskSnapshot {
        let technical = build_technical_snapshot(&item.candles);
        let fundamental = build_fundamental_snapshot(item);
        let mut signal_codes = Vec::new();
        let volatility_elevated = technical
            .atr
            .zip(item.price)
            .is_some_and(|(atr, price)| price > 0.0 && atr / price > 0.04);
        if volatility_elevated {
            signal_codes.push("volatility_elevated".to_string());
        }
        let liquidity_warning = item
            .candles
            .last()
            .is_some_and(|last| last.volume <= 100_000);
        if liquidity_warning {
            signal_codes.push("liquidity_warning".to_string());
        }
        let valuation_stretched = fundamental.pe_like.is_some_and(|value| value >= 45.0)
            || fundamental.ps_like.is_some_and(|value| value >= 10.0);
        if valuation_stretched {
            signal_codes.push("valuation_stretched".to_string());
        }
        let hard_negative_news = item
            .evidence_records
            .iter()
            .any(|record| record.hard_negative_flag);
        if hard_negative_news {
            signal_codes.push("hard_negative_news".to_string());
        }
        StockPickRiskSnapshot {
            hard_negative_news,
            volatility_elevated,
            liquidity_warning,
            valuation_stretched,
            signal_codes,
        }
    }

    pub(super) fn build_data_quality_snapshot(
        item: &EnrichedCandidate,
    ) -> StockPickDataQualitySnapshot {
        let quote_ready = item.price.is_some_and(|value| value > 0.0);
        let fundamentals_ready = item.fundamentals.is_some();
        let technical_ready = item.candles.len() >= 20;
        let news_ready = !item.news.is_empty() || !item.evidence_records.is_empty();
        let history_ready =
            !item.history_match_snapshot.enabled || item.history_match_snapshot.sample_count > 0;
        let vector_store_ready =
            !item.history_match_snapshot.enabled || item.history_match_snapshot.vector_hit_count > 0;
        let redis_ready =
            !item.history_match_snapshot.enabled || item.history_match_snapshot.sample_count > 0;
        let mut gaps = Vec::new();
        if !quote_ready {
            gaps.push("quote_missing".to_string());
        }
        if !fundamentals_ready {
            gaps.push("fundamentals_missing".to_string());
        }
        if !technical_ready {
            gaps.push("technical_history_missing".to_string());
        }
        if !news_ready {
            gaps.push("news_evidence_missing".to_string());
        }
        if !history_ready {
            gaps.push("history_missing".to_string());
        }
        let completeness_score = [
            quote_ready,
            fundamentals_ready,
            technical_ready,
            news_ready,
            history_ready,
            vector_store_ready,
            redis_ready,
        ]
        .into_iter()
        .filter(|value| *value)
        .count() as i32
            * 14;
        StockPickDataQualitySnapshot {
            quote_ready,
            fundamentals_ready,
            technical_ready,
            news_ready,
            history_ready,
            vector_store_ready,
            redis_ready,
            completeness_score,
            gaps,
        }
    }

    pub(super) fn build_technical_snapshot(candles: &[CandlePoint]) -> StockPickTechnicalSnapshot {
        StockPickTechnicalSnapshot {
            close_10_ema: TradingToolbox::ema(candles, 10),
            close_50_sma: TradingToolbox::sma(candles, 50),
            close_200_sma: TradingToolbox::sma(candles, 200),
            rsi: TradingToolbox::rsi(candles, 14),
            atr: TradingToolbox::atr(candles, 14),
            macd: TradingToolbox::macd(candles).map(|value| value.0),
            macd_signal: TradingToolbox::macd(candles).map(|value| value.1),
            macd_hist: TradingToolbox::macd(candles).map(|value| value.2),
            adx: TradingToolbox::adx(candles, 14),
            kdj_k: TradingToolbox::kdj(candles, 9).map(|value| value.0),
            kdj_d: TradingToolbox::kdj(candles, 9).map(|value| value.1),
            kdj_j: TradingToolbox::kdj(candles, 9).map(|value| value.2),
            cci: TradingToolbox::cci(candles, 20),
            wr: TradingToolbox::wr(candles, 14),
            obv: TradingToolbox::obv(candles).map(|value| value.0),
            vwap: TradingToolbox::vwap(candles, 20),
            vwma: TradingToolbox::vwma(candles, 20),
        }
    }
}

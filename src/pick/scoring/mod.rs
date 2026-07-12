use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Utc};
use futures::{StreamExt, stream};

use crate::data::{CandlePoint, FundamentalsSnapshot, MarketDataClient, NewsItem};
use crate::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
    StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
    StockPickTechnicalSnapshot,
};

use crate::pick::{CandidateContext, EnrichedCandidate, FactorBreakdown};

mod calc;
pub mod factors;
mod weights;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub(crate) use weights::apply_portfolio_constraints;

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

    weights::apply_cross_sectional_normalization(items);

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
// snapshots (inlined from scoring/snapshots.rs)
// ---------------------------------------------------------------------------

mod snapshots {
    use super::*;
    use calc::{
        adx_candles, atr_candles, candle_volume_ratio, cci_candles, ema_candles, kdj_candles,
        macd_candles, obv_candles, rsi_candles, sma_candles, vwap_candles, vwma_candles,
        wr_candles,
    };

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

    /// Attempt to parse a `published_at` string into a `NaiveDate`, trying
    /// several common formats found across news sources.
    fn parse_news_date(raw: &str) -> Option<NaiveDate> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        // ISO / GDELT style: "YYYY-MM-DD ..." or "YYYY-MM-DDTHH:MM:SS"
        if let Some(date_part) = trimmed.get(..10)
            && let Ok(d) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d")
        {
            return Some(d);
        }
        // US style: "M/D/YYYY" or "MM/DD/YYYY"
        if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%m/%d/%Y") {
            return Some(d);
        }
        // US style with time: "M/D/YYYY HH:MM:SS"
        if let Some(date_part) = trimmed.split_whitespace().next()
            && let Ok(d) = NaiveDate::parse_from_str(date_part, "%m/%d/%Y")
        {
            return Some(d);
        }
        // Dotted: "YYYY.MM.DD"
        if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%Y.%m.%d") {
            return Some(d);
        }
        if let Some(date_part) = trimmed.split_whitespace().next()
            && let Ok(d) = NaiveDate::parse_from_str(date_part, "%Y.%m.%d")
        {
            return Some(d);
        }
        // Long format: "Aug 10, 2020" / "January 5, 2024"
        if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%b %d, %Y") {
            return Some(d);
        }
        if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%B %d, %Y") {
            return Some(d);
        }
        // Try parsing a relative date string like "2 hours ago", "3 days ago"
        let lower = trimmed.to_ascii_lowercase();
        if let Some((amount_str, rest)) = lower.split_once(' ')
            && let Ok(amount) = amount_str.parse::<i64>()
        {
            let now = Utc::now().date_naive();
            if rest.starts_with("minute")
                || rest.starts_with("min")
                || rest.starts_with("hour")
                || rest.starts_with("hr")
            {
                return Some(now);
            } else if rest.starts_with("day") {
                return Some(now - chrono::Duration::days(amount));
            } else if rest.starts_with("week") {
                return Some(now - chrono::Duration::weeks(amount));
            } else if rest.starts_with("month") {
                return Some(now - chrono::Duration::days(amount * 30));
            } else if rest.starts_with("year") {
                return Some(now - chrono::Duration::days(amount * 365));
            }
        }
        if lower == "today" {
            return Some(Utc::now().date_naive());
        }
        if lower == "yesterday" {
            return Some(Utc::now().date_naive() - chrono::Duration::days(1));
        }
        None
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
            .filter_map(|n| parse_news_date(&n.published_at))
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
        let vector_ready = !item.history_match_snapshot.enabled
            || item.history_match_snapshot.vector_hit_count > 0;
        let cache_ready =
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
            vector_ready,
            cache_ready,
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
            vector_ready,
            cache_ready,
            completeness_score,
            gaps,
        }
    }

    pub(super) fn build_technical_snapshot(candles: &[CandlePoint]) -> StockPickTechnicalSnapshot {
        StockPickTechnicalSnapshot {
            close_10_ema: ema_candles(candles, 10),
            close_50_sma: sma_candles(candles, 50),
            close_200_sma: sma_candles(candles, 200),
            rsi: rsi_candles(candles, 14),
            atr: atr_candles(candles, 14),
            macd: macd_candles(candles).map(|value| value.0),
            macd_signal: macd_candles(candles).map(|value| value.1),
            macd_hist: macd_candles(candles).map(|value| value.2),
            adx: adx_candles(candles, 14),
            kdj_k: kdj_candles(candles, 9).map(|value| value.0),
            kdj_d: kdj_candles(candles, 9).map(|value| value.1),
            kdj_j: kdj_candles(candles, 9).map(|value| value.2),
            cci: cci_candles(candles, 20),
            wr: wr_candles(candles, 14),
            obv: obv_candles(candles).map(|value| value.0),
            vwap: vwap_candles(candles, 20),
            vwma: vwma_candles(candles, 20),
        }
    }
}

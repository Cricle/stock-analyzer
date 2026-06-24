use std::collections::HashSet;

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::data::{CandlePoint, NewsItem};
use crate::models::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickMarketSnapshot,
    StockPickNewsSnapshot, StockPickRiskSnapshot, StockPickTechnicalSnapshot,
};

use crate::engine::stock_pick::EnrichedCandidate;

use super::technicals::{
    adx_candles, atr_candles, candle_volume_ratio, cci_candles, ema_candles, kdj_candles,
    macd_candles, obv_candles, rsi_candles, sma_candles, vwap_candles, vwma_candles, wr_candles,
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
    let period_return_pct = item
        .candles
        .first()
        .zip(item.candles.last())
        .and_then(|(first, last)| {
            (first.close > Decimal::ZERO)
                .then_some(((last.close / first.close) - Decimal::ONE) * Decimal::from(100))
        })
        .map(|v| v.to_f64().unwrap_or_default());
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

pub(super) fn build_fundamental_snapshot(item: &EnrichedCandidate) -> StockPickFundamentalSnapshot {
    let Some(f) = item.fundamentals.as_ref() else {
        return StockPickFundamentalSnapshot {
            industry: item.industry.clone(),
            market_cap: item.market_cap,
            ..StockPickFundamentalSnapshot::default()
        };
    };
    let pe_like = match (
        f.market_cap.filter(|v| *v > Decimal::ZERO),
        f.net_income_usd.filter(|v| *v > Decimal::ZERO),
    ) {
        (Some(mc), Some(ni)) => Some((mc / ni).to_f64().unwrap_or_default()),
        _ => None,
    };
    let ps_like = match (
        f.market_cap.filter(|v| *v > Decimal::ZERO),
        f.revenues_usd.filter(|v| *v > Decimal::ZERO),
    ) {
        (Some(mc), Some(rev)) => Some((mc / rev).to_f64().unwrap_or_default()),
        _ => None,
    };
    let roe = match (
        f.net_income_usd,
        f.stockholders_equity_usd
            .filter(|value| *value > Decimal::ZERO),
    ) {
        (Some(ni), Some(eq)) => Some((ni / eq).to_f64().unwrap_or_default()),
        _ => None,
    };
    let leverage = match (
        f.total_debt_usd,
        f.stockholders_equity_usd
            .filter(|value| *value > Decimal::ZERO),
    ) {
        (Some(debt), Some(eq)) => Some((debt / eq).to_f64().unwrap_or_default()),
        _ => None,
    };
    StockPickFundamentalSnapshot {
        industry: f
            .industry
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| item.industry.clone()),
        market_cap: f
            .market_cap
            .map(|v| v.to_f64().unwrap_or_default())
            .or(item.market_cap),
        revenues_usd: f.revenues_usd.map(|v| v.to_f64().unwrap_or_default()),
        net_income_usd: f.net_income_usd.map(|v| v.to_f64().unwrap_or_default()),
        free_cash_flow_usd: f.free_cash_flow_usd.map(|v| v.to_f64().unwrap_or_default()),
        total_debt_usd: f.total_debt_usd.map(|v| v.to_f64().unwrap_or_default()),
        cash_and_equivalents_usd: f
            .cash_and_equivalents_usd
            .map(|v| v.to_f64().unwrap_or_default()),
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
    if let Some(date_part) = trimmed.get(..10) {
        if let Ok(d) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
            return Some(d);
        }
    }
    // US style: "M/D/YYYY" or "MM/DD/YYYY"
    if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%m/%d/%Y") {
        return Some(d);
    }
    // US style with time: "M/D/YYYY HH:MM:SS"
    if let Some(date_part) = trimmed.split_whitespace().next() {
        if let Ok(d) = NaiveDate::parse_from_str(date_part, "%m/%d/%Y") {
            return Some(d);
        }
    }
    // Dotted: "YYYY.MM.DD"
    if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%Y.%m.%d") {
        return Some(d);
    }
    if let Some(date_part) = trimmed.split_whitespace().next() {
        if let Ok(d) = NaiveDate::parse_from_str(date_part, "%Y.%m.%d") {
            return Some(d);
        }
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
    if let Some((amount_str, rest)) = lower.split_once(' ') {
        if let Ok(amount) = amount_str.parse::<i64>() {
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
    let qdrant_ready =
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
        qdrant_ready,
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
        qdrant_ready,
        redis_ready,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_parse_news_date_iso() {
        let date = parse_news_date("2024-01-15T10:30:00");
        assert_eq!(date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_parse_news_date_iso_date_only() {
        let date = parse_news_date("2024-01-15");
        assert_eq!(date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_parse_news_date_us_format() {
        let date = parse_news_date("1/15/2024");
        assert_eq!(date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_parse_news_date_us_padded() {
        let date = parse_news_date("01/15/2024");
        assert_eq!(date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_parse_news_date_us_with_time() {
        let date = parse_news_date("1/15/2024 10:30:00");
        assert_eq!(date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_parse_news_date_dotted() {
        let date = parse_news_date("2024.01.15");
        assert_eq!(date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_parse_news_date_dotted_with_time() {
        let date = parse_news_date("2024.01.15 10:30:00");
        assert_eq!(date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_parse_news_date_long_format() {
        let date = parse_news_date("Jan 15, 2024");
        assert_eq!(date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_parse_news_date_long_format_full() {
        let date = parse_news_date("January 15, 2024");
        assert_eq!(date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_parse_news_date_empty() {
        assert_eq!(parse_news_date(""), None);
        assert_eq!(parse_news_date("  "), None);
    }

    #[test]
    fn test_parse_news_date_invalid() {
        assert_eq!(parse_news_date("not a date"), None);
    }

    #[test]
    fn test_parse_news_date_today() {
        let date = parse_news_date("today");
        assert_eq!(date, Some(Utc::now().date_naive()));
    }

    #[test]
    fn test_parse_news_date_yesterday() {
        let date = parse_news_date("yesterday");
        assert_eq!(
            date,
            Some(Utc::now().date_naive() - chrono::Duration::days(1))
        );
    }

    #[test]
    fn test_parse_news_date_relative_days() {
        let date = parse_news_date("3 days ago");
        assert_eq!(
            date,
            Some(Utc::now().date_naive() - chrono::Duration::days(3))
        );
    }

    #[test]
    fn test_parse_news_date_relative_weeks() {
        let date = parse_news_date("2 weeks ago");
        assert_eq!(
            date,
            Some(Utc::now().date_naive() - chrono::Duration::weeks(2))
        );
    }

    #[test]
    fn test_parse_news_date_relative_hours() {
        let date = parse_news_date("5 hours ago");
        assert_eq!(date, Some(Utc::now().date_naive()));
    }

    #[test]
    fn test_format_relative_time_today() {
        let today = Utc::now().date_naive();
        assert_eq!(format_relative_time(today), "today");
    }

    #[test]
    fn test_format_relative_time_yesterday() {
        let yesterday = Utc::now().date_naive() - chrono::Duration::days(1);
        assert_eq!(format_relative_time(yesterday), "yesterday");
    }

    #[test]
    fn test_format_relative_time_days() {
        let date = Utc::now().date_naive() - chrono::Duration::days(3);
        assert_eq!(format_relative_time(date), "3 days ago");
    }

    #[test]
    fn test_format_relative_time_week() {
        let date = Utc::now().date_naive() - chrono::Duration::weeks(1);
        assert_eq!(format_relative_time(date), "1 week ago");
    }

    #[test]
    fn test_format_relative_time_weeks() {
        let date = Utc::now().date_naive() - chrono::Duration::weeks(3);
        assert_eq!(format_relative_time(date), "3 weeks ago");
    }

    #[test]
    fn test_format_relative_time_month() {
        let date = Utc::now().date_naive() - chrono::Duration::days(31);
        assert_eq!(format_relative_time(date), "1 month ago");
    }

    #[test]
    fn test_format_relative_time_months() {
        let date = Utc::now().date_naive() - chrono::Duration::days(90);
        assert_eq!(format_relative_time(date), "3 months ago");
    }

    #[test]
    fn test_format_relative_time_year() {
        let date = Utc::now().date_naive() - chrono::Duration::days(366);
        assert_eq!(format_relative_time(date), "1 year ago");
    }

    #[test]
    fn test_format_relative_time_future() {
        let future = Utc::now().date_naive() + chrono::Duration::days(5);
        let result = format_relative_time(future);
        assert!(result.contains("202") || result.contains("20"));
    }

    #[test]
    fn test_resolve_latest_published_at_empty() {
        let result = resolve_latest_published_at(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_resolve_latest_published_at_recent() {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let news = vec![NewsItem {
            published_at: today,
            title: "Test".to_string(),
            summary: "".to_string(),
            source: "test".to_string(),
            url: None,
        }];
        let result = resolve_latest_published_at(&news);
        assert_eq!(result, "today");
    }

    #[test]
    fn test_resolve_latest_published_at_old() {
        let old_date = (Utc::now() - chrono::Duration::days(200))
            .format("%Y-%m-%d")
            .to_string();
        let news = vec![NewsItem {
            published_at: old_date,
            title: "Old news".to_string(),
            summary: "".to_string(),
            source: "test".to_string(),
            url: None,
        }];
        let result = resolve_latest_published_at(&news);
        assert!(result.is_empty());
    }
}

use std::collections::HashSet;

use chrono::{NaiveDate, Utc};

use crate::data::{CandlePoint, NewsItem};
use crate::models::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickMarketSnapshot,
    StockPickNewsSnapshot, StockPickRiskSnapshot, StockPickTechnicalSnapshot,
};
use crate::engine::stock_pick::EnrichedCandidate;
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
    // Build enrichment line
    let mut enrich_parts = Vec::new();
    if let Some(pe_ttm) = item.fundamental_snapshot.pe_ttm {
        enrich_parts.push(format!("pe_ttm={:.1}", pe_ttm));
    }
    if let Some(pb) = item.fundamental_snapshot.pb {
        enrich_parts.push(format!("pb={:.1}", pb));
    }
    if let Some(peg) = item.fundamental_snapshot.peg {
        enrich_parts.push(format!("peg={:.2}", peg));
    }
    if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy {
        enrich_parts.push(format!("rev_yoy={:.1}%", rev_yoy * 100.0));
    }
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy {
        enrich_parts.push(format!("np_yoy={:.1}%", np_yoy * 100.0));
    }
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio {
        enrich_parts.push(format!("fund_flow={:.2}%", flow * 100.0));
    }
    if let Some(br) = item.fundamental_snapshot.analyst_buy_ratio {
        enrich_parts.push(format!("analyst_buy={:.0}%", br * 100.0));
    }
    let enrich_line = if enrich_parts.is_empty() {
        "none".to_string()
    } else {
        enrich_parts.join(", ")
    };

    format!(
        "Symbol: {}\nName: {}\nMarket: {} {}\nIndustry: {}\nPrice: {:?}\nDay Change: {:?}\nReturn Window: {:?}\nMarket Cap: {:?}\nVolume Ratio: {:?}\nFactor Scores: total={:.2}, momentum={:.2}, quality={:.2}, value={:.2}, growth={:.2}, profitability={:.2}, risk={:.2}, event={:.2}, evidence={:.2}, history={:.2}, penalty={:.2}\nTechnical: ema10={:?}, sma50={:?}, sma200={:?}, rsi={:?}, macd_hist={:?}, atr={:?}, adx={:?}, obv={:?}, vwap={:?}\nEnrichment: {}\nEvidence Count: {}\nHistory Samples: {}\nRejected Reasons: {}\nEvidence Headlines:\n{}",
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
        factor.growth,
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
        enrich_line,
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
        f.total_debt_usd
            .or(f.liabilities_usd),
        f.stockholders_equity_usd.filter(|value| *value > 0.0),
    ) {
        (Some(debt), Some(eq)) => Some(debt / eq),
        _ => {
            // Fallback: compute from assets and equity if both available
            match (
                f.assets_usd.filter(|v| *v > 0.0),
                f.stockholders_equity_usd.filter(|v| *v > 0.0),
            ) {
                (Some(assets), Some(eq)) => {
                    let liabilities = assets - eq;
                    if eq > 0.0 { Some(liabilities / eq) } else { None }
                }
                _ => None,
            }
        }
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
        pe_ttm: item.enrichment.pe_ttm,
        pb: item.enrichment.pb,
        peg: item.enrichment.peg,
        revenue_yoy: item.enrichment.revenue_yoy,
        net_profit_yoy: item.enrichment.net_profit_yoy,
        gross_margin: item.enrichment.gross_margin,
        fund_flow_net_ratio: item.enrichment.fund_flow_net_ratio,
        chip_benefit_ratio: item.enrichment.chip_benefit_ratio,
        chip_avg_cost: item.enrichment.chip_avg_cost,
        chip_concentration_90: item.enrichment.chip_concentration_90,
        dividend_yield: item.enrichment.dividend_yield,
        analyst_report_count: item.enrichment.analyst_report_count,
        analyst_buy_ratio: item.enrichment.analyst_buy_ratio,
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
    let pe_for_risk = fundamental
        .pe_ttm
        .filter(|v| *v > 0.0)
        .or(fundamental.pe_like);
    let valuation_stretched = pe_for_risk.is_some_and(|value| value >= 45.0)
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
    // Enrichment risk signals
    if fundamental.net_profit_yoy.is_some_and(|v| v < -0.2) {
        signal_codes.push("earnings_decline".to_string());
    }
    if fundamental.fund_flow_net_ratio.is_some_and(|v| v < -0.05) {
        signal_codes.push("fund_outflow".to_string());
    }
    if fundamental.chip_benefit_ratio.is_some_and(|v| v < 0.3) {
        signal_codes.push("low_chip_benefit".to_string());
    }
    if fundamental.leverage.is_some_and(|v| v > 2.0) {
        signal_codes.push("high_leverage".to_string());
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
    let enrichment_ready = item.enrichment.pe_ttm.is_some()
        || item.enrichment.pb.is_some()
        || item.enrichment.revenue_yoy.is_some()
        || item.enrichment.fund_flow_net_ratio.is_some();
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
    if !enrichment_ready {
        gaps.push("enrichment_missing".to_string());
    }
    let completeness_score = [
        quote_ready,
        fundamentals_ready,
        technical_ready,
        news_ready,
        history_ready,
        vector_store_ready,
        enrichment_ready,
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
        redis_ready: false,
        enrichment_ready,
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

use crate::data::{
    EarningsForecast, FundFlowEntry, HotStockXq, LhbStockStatistic, MarginRatioPa, ZtPool,
};

pub(super) fn trend_label(current: Option<f64>, reference: Option<f64>) -> &'static str {
    match current.zip(reference) {
        Some((price, level)) if price > level => "above",
        Some((price, level)) if price < level => "below",
        Some(_) => "at",
        None => "",
    }
}

pub(super) fn rsi_label(value: f64) -> &'static str {
    if value > 70.0 {
        "overbought"
    } else if value < 30.0 {
        "oversold"
    } else {
        "neutral"
    }
}

pub(super) fn adx_label(value: f64) -> &'static str {
    if value >= 25.0 {
        "strong_trend"
    } else if value <= 20.0 {
        "range_bound"
    } else {
        "moderate"
    }
}

pub(super) fn format_fund_flow_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<FundFlowEntry>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let lines: Vec<String> = items
                .iter()
                .take(5)
                .map(|item| {
                    let price = item
                        .latest_price
                        .map(|v| format!("{:.2}", v))
                        .unwrap_or_else(|| "-".into());
                    let chg = item
                        .change_pct
                        .map(|v| format!("{:.2}", v))
                        .unwrap_or_else(|| "-".into());
                    let net = item
                        .net_flow
                        .map(|v| format!("{:.0}", v))
                        .unwrap_or_else(|| "-".into());
                    let to = item
                        .turnover_rate
                        .map(|v| format!("{:.2}", v))
                        .unwrap_or_else(|| "-".into());
                    format!(
                        "{}: price={} chg={}% net_flow={} turnover={}%",
                        item.code, price, chg, net, to
                    )
                })
                .collect();
            format!("Fund Flow:\n{}", lines.join("\n"))
        }
        Ok(Ok(_)) => "Fund Flow: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("fund flow fetch failed for {}: {}", symbol, e);
            "Fund Flow: fetch failed".to_string()
        }
        Err(_) => "Fund Flow: timeout".to_string(),
    }
}

pub(super) fn format_billboard_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<LhbStockStatistic>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let lines: Vec<String> = items.iter().take(3).map(|item| {
                format!("{} {} date={} chg={:.2}% net={:.0} billboard_times={} org_buy={} org_sell={}",
                    item.code, item.name, item.latest_trade_date, item.change_pct,
                    item.billboard_net_amount, item.billboard_times, item.org_buy_times, item.org_sell_times)
            }).collect();
            format!("Billboard:\n{}", lines.join("\n"))
        }
        Ok(Ok(_)) => "Billboard: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("billboard fetch failed for {}: {}", symbol, e);
            "Billboard: fetch failed".to_string()
        }
        Err(_) => "Billboard: timeout".to_string(),
    }
}

pub(super) fn format_margin_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<MarginRatioPa>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let lines: Vec<String> = items
                .iter()
                .take(3)
                .map(|item| {
                    format!(
                        "{} {}: fin_ratio={:.4} loan_ratio={:.4}",
                        item.code, item.name, item.fin_ratio, item.loan_ratio
                    )
                })
                .collect();
            format!("Margin Trading:\n{}", lines.join("\n"))
        }
        Ok(Ok(_)) => "Margin: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("margin fetch failed for {}: {}", symbol, e);
            "Margin: fetch failed".to_string()
        }
        Err(_) => "Margin: timeout".to_string(),
    }
}

pub(super) fn format_hot_rank_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<HotStockXq>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let lines: Vec<String> = items
                .iter()
                .take(3)
                .map(|item| {
                    format!(
                        "{} {} value={:.0} price={:.2}",
                        item.code, item.name, item.value, item.latest_price
                    )
                })
                .collect();
            format!("Hot Rank:\n{}", lines.join("\n"))
        }
        Ok(Ok(_)) => "Hot Rank: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("hot rank fetch failed for {}: {}", symbol, e);
            "Hot Rank: fetch failed".to_string()
        }
        Err(_) => "Hot Rank: timeout".to_string(),
    }
}

pub(super) fn format_earnings_forecast_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<EarningsForecast>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let matching: Vec<&EarningsForecast> =
                items.iter().filter(|item| item.code == symbol).collect();
            if matching.is_empty() {
                format!(
                    "Earnings Forecast: {} entries on date, none for {}",
                    items.len(),
                    symbol
                )
            } else {
                let lines: Vec<String> = matching
                    .iter()
                    .take(3)
                    .map(|item| {
                        format!(
                            "{} {} type={} content={} range={}",
                            item.code,
                            item.name,
                            item.forecast_type,
                            item.forecast_content.as_deref().unwrap_or("-"),
                            item.change_range.as_deref().unwrap_or("-")
                        )
                    })
                    .collect();
                format!("Earnings Forecast:\n{}", lines.join("\n"))
            }
        }
        Ok(Ok(_)) => "Earnings Forecast: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("earnings forecast fetch failed for {}: {}", symbol, e);
            "Earnings Forecast: fetch failed".to_string()
        }
        Err(_) => "Earnings Forecast: timeout".to_string(),
    }
}

pub(super) fn format_limit_pool_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<ZtPool>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let matching: Vec<_> = items.iter().filter(|item| item.code == symbol).collect();
            if matching.is_empty() {
                format!(
                    "Limit-Up Pool: {} stocks hit limit up, {} not in pool",
                    items.len(),
                    symbol
                )
            } else {
                let lines: Vec<String> = matching
                    .iter()
                    .take(3)
                    .map(|item| {
                        format!(
                            "{} {} chg={:.2}% price={:.2} seals={} consec={} industry={}",
                            item.code,
                            item.name,
                            item.change_pct,
                            item.latest_price,
                            item.seal_amount,
                            item.consecutive_count,
                            item.industry
                        )
                    })
                    .collect();
                format!("Limit-Up Pool:\n{}", lines.join("\n"))
            }
        }
        Ok(Ok(_)) => "Limit-Up Pool: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("zt pool fetch failed for {}: {}", symbol, e);
            "Limit-Up Pool: fetch failed".to_string()
        }
        Err(_) => "Limit-Up Pool: timeout".to_string(),
    }
}

pub(super) fn build_technical_summary(chart: &crate::models::ReportMarketChart) -> String {
    if chart.candles.is_empty() {
        return "Technical Indicators: no candle data".to_string();
    }
    let current_price = chart.candles.last().map(|c| c.close);
    let candles = &chart.candles;
    let sma50 = crate::models::sma_report(candles, 50);
    let sma200 = crate::models::sma_report(candles, 200);
    let ema10 = crate::models::ema_report(candles, 10);
    let rsi14 = crate::models::rsi_report(candles, 14);
    let atr14 = crate::models::atr_report(candles, 14);
    let macd_vals = crate::models::macd_report(candles);
    let boll = crate::models::bollinger_report(candles, 20);
    let kdj = crate::models::kdj_report(candles, 9);
    let adx14 = crate::models::adx_report(candles, 14);
    let obv = crate::models::obv_report(candles);

    let mut lines = Vec::new();
    if let Some(price) = current_price {
        lines.push(format!("price={:.2}", price));
    }
    if let Some(v) = sma50 {
        lines.push(format!(
            "SMA50={:.2} {}",
            v,
            trend_label(current_price, Some(v))
        ));
    }
    if let Some(v) = sma200 {
        lines.push(format!(
            "SMA200={:.2} {}",
            v,
            trend_label(current_price, Some(v))
        ));
    }
    if let Some(v) = ema10 {
        lines.push(format!(
            "EMA10={:.2} {}",
            v,
            trend_label(current_price, Some(v))
        ));
    }
    if let Some(v) = rsi14 {
        lines.push(format!("RSI14={:.1} {}", v, rsi_label(v)));
    }
    if let Some(v) = atr14 {
        lines.push(format!("ATR14={:.2}", v));
    }
    if let Some((macd, signal, hist)) = macd_vals {
        lines.push(format!(
            "MACD={:.4} signal={:.4} hist={:.4}",
            macd, signal, hist
        ));
    }
    if let Some((mid, upper, lower)) = boll {
        lines.push(format!("BOLL={:.2}/{:.2}/{:.2}", upper, mid, lower));
    }
    if let Some((k, d, j)) = kdj {
        lines.push(format!("KDJ={:.1}/{:.1}/{:.1}", k, d, j));
    }
    if let Some(v) = adx14 {
        lines.push(format!("ADX14={:.1} {}", v, adx_label(v)));
    }
    if let Some((obv_val, delta)) = obv {
        lines.push(format!(
            "OBV={:.0} delta={:.0} {}",
            obv_val,
            delta,
            if delta > 0.0 {
                "accumulation"
            } else {
                "distribution"
            }
        ));
    }
    format!("Technical Indicators:\n{}", lines.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trend_label_above() {
        assert_eq!(trend_label(Some(110.0), Some(100.0)), "above");
    }

    #[test]
    fn test_trend_label_below() {
        assert_eq!(trend_label(Some(90.0), Some(100.0)), "below");
    }

    #[test]
    fn test_trend_label_at() {
        assert_eq!(trend_label(Some(100.0), Some(100.0)), "at");
    }

    #[test]
    fn test_trend_label_none_current() {
        assert_eq!(trend_label(None, Some(100.0)), "");
    }

    #[test]
    fn test_trend_label_none_reference() {
        assert_eq!(trend_label(Some(100.0), None), "");
    }

    #[test]
    fn test_trend_label_both_none() {
        assert_eq!(trend_label(None, None), "");
    }

    #[test]
    fn test_rsi_label_overbought() {
        assert_eq!(rsi_label(75.0), "overbought");
        assert_eq!(rsi_label(90.0), "overbought");
    }

    #[test]
    fn test_rsi_label_oversold() {
        assert_eq!(rsi_label(25.0), "oversold");
        assert_eq!(rsi_label(10.0), "oversold");
    }

    #[test]
    fn test_rsi_label_neutral() {
        assert_eq!(rsi_label(50.0), "neutral");
        assert_eq!(rsi_label(70.0), "neutral");
        assert_eq!(rsi_label(30.0), "neutral");
    }

    #[test]
    fn test_adx_label_strong() {
        assert_eq!(adx_label(30.0), "strong_trend");
        assert_eq!(adx_label(50.0), "strong_trend");
    }

    #[test]
    fn test_adx_label_range_bound() {
        assert_eq!(adx_label(15.0), "range_bound");
        assert_eq!(adx_label(20.0), "range_bound");
    }

    #[test]
    fn test_adx_label_moderate() {
        assert_eq!(adx_label(22.0), "moderate");
    }
}

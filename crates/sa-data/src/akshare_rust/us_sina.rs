use crate::{CandlePoint, MarketDataClient, QuoteSnapshot, f64_to_dec};

pub(crate) async fn fetch_quote(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<QuoteSnapshot> {
    let mut items = fetch_candles(client, symbol, 2).await?;
    let last = items
        .pop()
        .ok_or_else(|| anyhow::anyhow!("sina us daily returned no rows for quote"))?;
    Ok(QuoteSnapshot {
        symbol: symbol.trim().to_uppercase(),
        date: last.trade_date,
        open: last.open,
        high: last.high,
        low: last.low,
        close: last.close,
        volume: last.volume,
    })
}

pub(crate) async fn fetch_candles(
    client: &MarketDataClient,
    symbol: &str,
    limit: usize,
) -> anyhow::Result<Vec<CandlePoint>> {
    let raw = client
        .ak
        .sina_us_daily(symbol, limit)
        .await
        .map_err(|e| anyhow::anyhow!("sina us daily error: {e}"))?;
    if raw.is_empty() {
        anyhow::bail!("sina us daily returned no rows");
    }
    let items: Vec<CandlePoint> = raw
        .into_iter()
        .map(|p| CandlePoint {
            trade_date: p.trade_date,
            open: f64_to_dec(p.open),
            close: f64_to_dec(p.close),
            high: f64_to_dec(p.high),
            low: f64_to_dec(p.low),
            volume: p.volume,
            amount: f64_to_dec(p.amount),
            amplitude_pct: p.amplitude_pct,
            change_pct: p.change_pct,
            change_amount: f64_to_dec(p.change_amount),
            turnover_pct: p.turnover_pct,
        })
        .collect();
    Ok(items)
}

use super::types::ProviderResult;
use crate::data::{
    CandlePoint, FundamentalsSnapshot, MarketDataClient, NewsItem, QuoteSnapshot,
};

use akshare::stock::us_extra::{UsFamousStock, UsPinkStock, UsSpotSina, UsValuationBaidu};
use akshare::stock::xueqiu::XqStockSpot;

// ---------------------------------------------------------------------------
// Core data
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_quote(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<ProviderResult<QuoteSnapshot>> {
    let ak_quote = client.ak.us_quote(symbol).await?;
    Ok((super::quote_from_akshare(ak_quote), "akshare".to_string()))
}

pub(crate) async fn fetch_fundamentals(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<FundamentalsSnapshot> {
    client.fetch_us_fundamentals(symbol).await
}

pub(crate) async fn fetch_insider_transactions(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<NewsItem>> {
    client.fetch_us_insider_transactions(symbol).await
}

pub(crate) async fn fetch_candles(
    client: &MarketDataClient,
    symbol: &str,
    limit: usize,
) -> anyhow::Result<ProviderResult<Vec<CandlePoint>>> {
    let ak_candles = client.ak.us_candles(symbol, limit).await?;
    Ok((ak_candles.into_iter().map(super::candle_from_akshare).collect(), "akshare".to_string()))
}

pub(crate) async fn fetch_return_since(
    client: &MarketDataClient,
    symbol: &str,
    start_date: &str,
    holding_days: usize,
) -> anyhow::Result<Option<f64>> {
    match client
        .fetch_us_return_since(symbol, start_date, holding_days)
        .await
    {
        Ok(value) => Ok(value),
        Err(error) => {
            tracing::info!(
                symbol = %symbol,
                start_date = %start_date,
                holding_days,
                error = ?error,
                "US return_since upstream unavailable, skipping outcome computation"
            );
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// US spot (Sina / Eastmoney)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_us_spot(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<UsSpotSina>> {
    client.ak.stock_us_spot().await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// US famous stocks (Eastmoney)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_us_famous_spot(
    client: &MarketDataClient,
    category: &str,
) -> anyhow::Result<Vec<UsFamousStock>> {
    client
        .ak
        .stock_us_famous_spot_em(category)
        .await
        .map_err(Into::into)
}

// ---------------------------------------------------------------------------
// US pink sheet stocks (Eastmoney)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_us_pink_spot(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<UsPinkStock>> {
    client.ak.stock_us_pink_spot_em().await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// US valuation (Baidu)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_us_valuation(
    client: &MarketDataClient,
    symbol: &str,
    indicator: &str,
    period: &str,
) -> anyhow::Result<Vec<UsValuationBaidu>> {
    client
        .ak
        .stock_us_valuation_baidu(symbol, indicator, period)
        .await
        .map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Xueqiu spot (works for US symbols)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_xq_spot(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<XqStockSpot> {
    client
        .ak
        .stock_individual_spot_xq(symbol)
        .await
        .map_err(Into::into)
}

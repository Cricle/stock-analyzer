pub(crate) mod a_share;
pub(crate) mod hk;
mod types;
pub(crate) mod us;
pub(crate) mod us_sina;

use super::{
    CandlePoint, FundamentalsSnapshot, MarketDataClient, MarketKind, NewsItem, QuoteSnapshot,
    StockSearchResult,
};
use types::ProviderResult;

pub(crate) fn quote_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:tencent_quote+tushare_daily",
        MarketKind::HongKong => "akshare:tencent_quote+yahoo_finance_chart",
        MarketKind::UsEquity => "akshare:sina_us_daily+yahoo_finance_chart+stooq",
    }
}

pub(crate) fn fundamentals_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:eastmoney+tushare",
        MarketKind::HongKong => "akshare:tencent_quote+eastmoney_search",
        MarketKind::UsEquity => "akshare:sec_edgar",
    }
}

pub(crate) fn news_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:eastmoney+searxng_news",
        MarketKind::HongKong => "akshare:searxng_news",
        MarketKind::UsEquity => "akshare:sec_edgar+searxng_news",
    }
}

pub(crate) fn candles_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:tencent_kline+eastmoney_kline",
        MarketKind::HongKong => "akshare:tencent_kline+yahoo_finance_chart",
        MarketKind::UsEquity => "akshare:sina_us_daily+yahoo_finance_chart+stooq",
    }
}

pub(crate) async fn fetch_quote(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<ProviderResult<QuoteSnapshot>> {
    match client.detect_market(symbol) {
        MarketKind::AShare => {
            let ts_code = client
                .normalize_a_share_symbol(symbol)
                .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol"))?;
            a_share::fetch_quote(client, symbol, &ts_code).await
        }
        MarketKind::HongKong => hk::fetch_quote(client, symbol).await,
        MarketKind::UsEquity => us::fetch_quote(client, symbol).await,
    }
}

pub(crate) async fn fetch_fundamentals(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<FundamentalsSnapshot> {
    match client.detect_market(symbol) {
        MarketKind::AShare => {
            let ts_code = client
                .normalize_a_share_symbol(symbol)
                .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol"))?;
            a_share::fetch_fundamentals(client, symbol, &ts_code).await
        }
        MarketKind::HongKong => hk::fetch_fundamentals(client, symbol).await,
        MarketKind::UsEquity => us::fetch_fundamentals(client, symbol).await,
    }
}

pub(crate) async fn fetch_insider_transactions(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<NewsItem>> {
    match client.detect_market(symbol) {
        MarketKind::AShare => a_share::fetch_insider_transactions(client, symbol).await,
        MarketKind::HongKong => hk::fetch_insider_transactions(client, symbol).await,
        MarketKind::UsEquity => us::fetch_insider_transactions(client, symbol).await,
    }
}

pub(crate) async fn fetch_candles(
    client: &MarketDataClient,
    symbol: &str,
    adjust: &str,
    limit: usize,
) -> anyhow::Result<ProviderResult<Vec<CandlePoint>>> {
    match client.detect_market(symbol) {
        MarketKind::AShare => a_share::fetch_candles(client, symbol, adjust, limit).await,
        MarketKind::HongKong => hk::fetch_candles(client, symbol, limit).await,
        MarketKind::UsEquity => us::fetch_candles(client, symbol, limit).await,
    }
}

pub(crate) async fn search_stocks(
    client: &MarketDataClient,
    query: &str,
    market: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<StockSearchResult>> {
    a_share::search_stocks(client, query, market, limit).await
}

pub(crate) async fn fetch_return_since(
    client: &MarketDataClient,
    symbol: &str,
    start_date: &str,
    holding_days: usize,
) -> anyhow::Result<Option<f64>> {
    match client.detect_market(symbol) {
        MarketKind::AShare => {
            a_share::fetch_return_since(client, symbol, start_date, holding_days).await
        }
        MarketKind::HongKong => {
            hk::fetch_return_since(client, symbol, start_date, holding_days).await
        }
        MarketKind::UsEquity => {
            us::fetch_return_since(client, symbol, start_date, holding_days).await
        }
    }
}

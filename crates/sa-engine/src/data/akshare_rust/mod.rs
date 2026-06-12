pub(crate) mod a_share;
pub(crate) mod hk;
mod types;
pub(crate) mod us;

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

use super::{
    CandlePoint, CapitalFlowPoint, FundamentalsSnapshot, MarketDataClient, MarketKind, NewsItem,
    QuoteSnapshot, StockSearchResult,
};
use types::ProviderResult;

// ---------------------------------------------------------------------------
// Conversion utilities from akshare f64-based types to our Decimal-based types.
// ---------------------------------------------------------------------------

fn f64_to_dec(v: f64) -> Decimal {
    Decimal::from_f64(v).unwrap_or_default()
}

pub(crate) fn quote_from_akshare(q: akshare::QuoteSnapshot) -> QuoteSnapshot {
    QuoteSnapshot {
        symbol: q.symbol,
        date: q.date,
        open: f64_to_dec(q.open),
        high: f64_to_dec(q.high),
        low: f64_to_dec(q.low),
        close: f64_to_dec(q.close),
        volume: q.volume,
    }
}

pub(crate) fn candle_from_akshare(c: akshare::CandlePoint) -> CandlePoint {
    CandlePoint {
        trade_date: c.trade_date,
        open: f64_to_dec(c.open),
        close: f64_to_dec(c.close),
        high: f64_to_dec(c.high),
        low: f64_to_dec(c.low),
        volume: c.volume,
        amount: f64_to_dec(c.amount),
        amplitude_pct: c.amplitude_pct,
        change_pct: c.change_pct,
        change_amount: f64_to_dec(c.change_amount),
        turnover_pct: c.turnover_pct,
    }
}

pub(crate) fn capital_flow_from_akshare(c: akshare::CapitalFlowPoint) -> CapitalFlowPoint {
    CapitalFlowPoint {
        trade_date: c.trade_date,
        main_net_inflow: f64_to_dec(c.main_net_inflow),
        small_net_inflow: f64_to_dec(c.small_net_inflow),
        medium_net_inflow: f64_to_dec(c.medium_net_inflow),
        large_net_inflow: f64_to_dec(c.large_net_inflow),
        super_large_net_inflow: f64_to_dec(c.super_large_net_inflow),
        main_net_inflow_ratio_pct: c.main_net_inflow_ratio_pct,
        small_net_inflow_ratio_pct: c.small_net_inflow_ratio_pct,
        medium_net_inflow_ratio_pct: c.medium_net_inflow_ratio_pct,
        large_net_inflow_ratio_pct: c.large_net_inflow_ratio_pct,
        super_large_net_inflow_ratio_pct: c.super_large_net_inflow_ratio_pct,
        close: f64_to_dec(c.close),
        change_pct: c.change_pct,
    }
}

pub(crate) fn news_item_from_akshare(n: akshare::NewsItem) -> NewsItem {
    NewsItem {
        published_at: n.published_at,
        title: n.title,
        summary: n.summary,
        source: n.source,
        url: n.url,
    }
}

pub(crate) fn news_item_from_announcement(a: akshare::AnnouncementItem) -> NewsItem {
    NewsItem {
        published_at: a.published_at,
        title: a.title.clone(),
        summary: a.title,
        source: a.source,
        url: a.url,
    }
}

pub(crate) fn news_item_from_stock_news(n: akshare::stock::feature::StockNews) -> NewsItem {
    NewsItem {
        published_at: n.publish_time,
        title: n.title.clone(),
        summary: n.content.unwrap_or(n.title),
        source: n.source.unwrap_or_else(|| "Eastmoney".to_string()),
        url: n.url,
    }
}

pub(crate) fn balance_sheet_to_wire(
    b: &akshare::stock::feature::BalanceSheet,
) -> super::wire::EastmoneyBalanceSheetItem {
    super::wire::EastmoneyBalanceSheetItem {
        report_date: b.notice_date.clone(),
        total_assets: b.total_assets,
        total_liabilities: b.total_liabilities,
        total_equity: b.equity,
        monetary_funds: b.cash,
        current_liab: None,
        totalnoncliab: None,
    }
}

pub(crate) fn cashflow_to_wire(
    c: &akshare::stock::feature::CashFlowSheet,
) -> super::wire::EastmoneyCashflowItem {
    super::wire::EastmoneyCashflowItem {
        report_date: c.notice_date.clone(),
        netcash_operate: c.operating_cash_flow,
        construct_long_asset: c.investing_cash_flow,
        end_cce: c.cash_increase,
    }
}

pub(crate) fn profit_sheet_to_wire(
    p: &akshare::stock::feature::ProfitSheet,
) -> super::wire::ProfitSheetWire {
    super::wire::ProfitSheetWire {
        notice_date: p.notice_date.clone(),
        total_revenue: p.total_revenue,
        net_profit: p.net_profit,
        net_profit_deducted: p.net_profit_deducted,
    }
}

pub(crate) fn quote_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:tencent_quote+eastmoney",
        MarketKind::HongKong => "akshare:tencent_quote+yahoo_finance_chart",
        MarketKind::UsEquity => "akshare:sina_us_daily+yahoo_finance_chart+stooq",
    }
}

pub(crate) fn fundamentals_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:eastmoney",
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

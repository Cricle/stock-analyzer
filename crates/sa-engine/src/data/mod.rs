use std::fmt;

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

use crate::types::NewsFetchAttempt;
pub use crate::types::{
    CandlePoint, CapitalFlowPoint, FundamentalsSnapshot, MarketKind, NewsItem, QuoteSnapshot,
};

/// Convert an `f64` to `Decimal`, returning `Decimal::ZERO` for `NaN`/`Inf`.
pub(crate) fn f64_to_dec(v: f64) -> Decimal {
    Decimal::from_f64(v).unwrap_or_default()
}

/// Map an `Option<f64>` to `Option<Decimal>`.
pub(crate) fn opt_f64_to_dec(v: Option<f64>) -> Option<Decimal> {
    v.map(f64_to_dec)
}

// ---------------------------------------------------------------------------
// Conversion from akshare-rs types to our domain types.
// ---------------------------------------------------------------------------

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

pub(crate) fn news_item_from_news_entry(n: akshare::stock::feature::NewsEntry) -> NewsItem {
    NewsItem {
        published_at: n.time,
        title: n.title.clone(),
        summary: n.summary.unwrap_or(n.title),
        source: String::new(),
        url: n.url,
    }
}

pub(crate) fn balance_sheet_to_wire(
    b: &akshare::stock::feature::BalanceSheet,
) -> wire::EastmoneyBalanceSheetItem {
    wire::EastmoneyBalanceSheetItem {
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
) -> wire::EastmoneyCashflowItem {
    wire::EastmoneyCashflowItem {
        report_date: c.notice_date.clone(),
        netcash_operate: c.operating_cash_flow,
        construct_long_asset: c.investing_cash_flow,
        end_cce: c.cash_increase,
    }
}

pub(crate) fn profit_sheet_to_wire(
    p: &akshare::stock::feature::ProfitSheet,
) -> wire::ProfitSheetWire {
    wire::ProfitSheetWire {
        notice_date: p.notice_date.clone(),
        total_revenue: p.total_revenue,
        net_profit: p.net_profit,
        net_profit_deducted: p.net_profit_deducted,
    }
}

pub use akshare::stock::feature::{
    EarningsForecast, FundFlowEntry, HotStockXq, LhbStockStatistic, MarginRatioPa, ZtPool,
};


mod a_share;
mod cache;
mod client;
pub mod diagnosis;
mod hk;
pub(crate) mod news_filter;
pub(crate) mod news_utils;
mod us;
mod wire;

/// Configuration for constructing a `MarketDataClient`.
/// The backend builds this from its `Settings`.
pub struct DataConfig {
}

#[derive(Clone)]
pub struct MarketDataClient {
    ak: akshare::AkShareClient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataErrorKind {
    UnsupportedMarket,
    PermissionDenied,
    Restricted,
    MissingCredentials,
    NotFound,
    Upstream,
}
impl DataErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedMarket => "unsupported_market",
            Self::PermissionDenied => "permission_denied",
            Self::Restricted => "restricted",
            Self::MissingCredentials => "missing_credentials",
            Self::NotFound => "not_found",
            Self::Upstream => "upstream_error",
        }
    }
}

#[derive(Debug)]
pub struct DataError {
    kind: DataErrorKind,
    message: String,
}

impl DataError {
    fn new(kind: DataErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DataError {}

pub(crate) fn news_result_cacheable(items: &[NewsItem], attempts: &[NewsFetchAttempt]) -> bool {
    !items.is_empty() && !attempts.is_empty() && attempts.iter().all(|attempt| attempt.success)
}

pub use akshare::types::BillboardEntry;

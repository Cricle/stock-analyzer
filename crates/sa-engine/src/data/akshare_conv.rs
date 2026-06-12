//! Conversion utilities from akshare f64-based types to our Decimal-based types.

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

use super::{CandlePoint, CapitalFlowPoint, QuoteSnapshot};

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

pub(crate) fn news_item_from_akshare(n: akshare::NewsItem) -> super::NewsItem {
    super::NewsItem {
        published_at: n.published_at,
        title: n.title,
        summary: n.summary,
        source: n.source,
        url: n.url,
    }
}

pub(crate) fn news_item_from_announcement(a: akshare::AnnouncementItem) -> super::NewsItem {
    super::NewsItem {
        published_at: a.published_at,
        title: a.title.clone(),
        summary: a.title,
        source: a.source,
        url: a.url,
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

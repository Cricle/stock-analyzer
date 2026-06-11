//! Market data primitive types for stock analysis.
//!
//! This crate provides the core data structures used throughout the stock-analyzer
//! ecosystem for representing market quotes, fundamentals, news, candlestick data,
//! and capital flow information.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// The stock market classification.
///
/// Distinguishes between the major market regions supported by the analyzer,
/// allowing downstream logic (data providers, scheduling, currency handling)
/// to branch on market identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketKind {
    /// Mainland China A-share market (Shanghai / Shenzhen).
    AShare,
    /// Hong Kong Stock Exchange.
    HongKong,
    /// US equity markets (NYSE / NASDAQ).
    UsEquity,
}

/// A point-in-time snapshot of a security's latest quote.
///
/// Captures the essential OHLCV (open-high-close-low-volume) data for a
/// single trading session, identified by symbol and date string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteSnapshot {
    /// Ticker symbol (e.g. `"AAPL"`, `"600519.SH"`).
    pub symbol: String,
    /// Trading date in `YYYY-MM-DD` format.
    pub date: String,
    /// Opening price.
    pub open: Decimal,
    /// Intraday high price.
    pub high: Decimal,
    /// Intraday low price.
    pub low: Decimal,
    /// Closing price.
    pub close: Decimal,
    /// Total shares traded during the session.
    pub volume: i64,
}

/// Fundamental financial data for a single company.
///
/// Contains both identity fields (symbol, company name, CIK) and a broad
/// set of optional financial metrics covering income, balance-sheet, and
/// cash-flow items denominated in USD.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundamentalsSnapshot {
    /// Ticker symbol.
    pub symbol: String,
    /// Full legal or commonly-known company name.
    pub company_name: String,
    /// SEC Central Index Key (CIK) used in EDGAR filings.
    pub cik: String,
    /// Industry or sector classification, if available.
    pub industry: Option<String>,
    /// Reporting currency code (e.g. `"USD"`, `"CNY"`).
    pub currency: String,
    /// Month/day on which the fiscal year ends (e.g. `"12-31"`), if known.
    pub fiscal_year_end: Option<String>,
    /// Total shares outstanding.
    pub shares_outstanding: Option<i64>,
    /// Market capitalization (price * shares outstanding).
    pub market_cap: Option<Decimal>,
    /// Net income in USD.
    pub net_income_usd: Option<Decimal>,
    /// Total revenues in USD.
    pub revenues_usd: Option<Decimal>,
    /// Total assets in USD.
    pub assets_usd: Option<Decimal>,
    /// Total liabilities in USD.
    pub liabilities_usd: Option<Decimal>,
    /// Stockholders' equity in USD.
    pub stockholders_equity_usd: Option<Decimal>,
    /// Cash and cash equivalents in USD.
    pub cash_and_equivalents_usd: Option<Decimal>,
    /// Gross profit in USD.
    pub gross_profit_usd: Option<Decimal>,
    /// Operating income in USD.
    pub operating_income_usd: Option<Decimal>,
    /// Total operating expenses in USD.
    pub operating_expenses_usd: Option<Decimal>,
    /// Cash flow from operations in USD.
    pub operating_cash_flow_usd: Option<Decimal>,
    /// Capital expenditure in USD.
    pub capital_expenditure_usd: Option<Decimal>,
    /// Free cash flow (operating cash flow minus capex) in USD.
    pub free_cash_flow_usd: Option<Decimal>,
    /// Long-term debt in USD.
    pub long_term_debt_usd: Option<Decimal>,
    /// Current (short-term) debt in USD.
    pub current_debt_usd: Option<Decimal>,
    /// Total debt (short-term + long-term) in USD.
    pub total_debt_usd: Option<Decimal>,
    /// Diluted shares outstanding.
    pub diluted_shares_outstanding: Option<i64>,
}

/// A single news article or headline related to a security.
///
/// Typically returned by news-aggregator APIs and used to feed sentiment
/// analysis or event-detection pipelines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    /// ISO-8601 timestamp of when the article was published.
    pub published_at: String,
    /// Headline or title of the article.
    pub title: String,
    /// Brief summary or snippet of the article content.
    pub summary: String,
    /// Name of the news outlet or data provider (e.g. `"Reuters"`).
    pub source: String,
    /// Direct URL to the full article, if available.
    pub url: Option<String>,
}

/// Metadata about a single attempt to fetch news from a provider.
///
/// Used for observability: the caller can inspect which sources succeeded,
/// how many items they returned, and what errors occurred.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewsFetchAttempt {
    /// Identifier of the news provider that was queried.
    pub source: String,
    /// The search query or ticker used for the request, if any.
    #[serde(default)]
    pub query: Option<String>,
    /// Whether the fetch attempt completed without error.
    pub success: bool,
    /// Number of news items returned by this attempt.
    #[serde(default)]
    pub item_count: usize,
    /// Error message if the attempt failed, `None` on success.
    #[serde(default)]
    pub error: Option<String>,
}

/// Aggregated result of one or more news-fetch attempts.
///
/// Combines the successfully retrieved [`NewsItem`]s with per-provider
/// [`NewsFetchAttempt`] diagnostics and a flag indicating whether the
/// combined result can be cached.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewsFetchResult {
    /// All successfully retrieved news items across providers.
    #[serde(default)]
    pub items: Vec<NewsItem>,
    /// Per-provider fetch attempt metadata.
    #[serde(default)]
    pub attempts: Vec<NewsFetchAttempt>,
    /// Whether this result set is safe to cache (no transient errors).
    #[serde(default)]
    pub cacheable: bool,
}

/// A single OHLCV candlestick with additional derived metrics.
///
/// Represents one trading day of price and volume data along with
/// computed fields such as amplitude, change percentage, and turnover.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlePoint {
    /// Trading date in `YYYY-MM-DD` format.
    pub trade_date: String,
    /// Opening price.
    pub open: Decimal,
    /// Closing price.
    pub close: Decimal,
    /// Intraday high price.
    pub high: Decimal,
    /// Intraday low price.
    pub low: Decimal,
    /// Total shares traded.
    pub volume: i64,
    /// Total turnover in currency units (price * volume).
    pub amount: Decimal,
    /// Intraday amplitude as a percentage `(high - low) / prev_close * 100`.
    pub amplitude_pct: f64,
    /// Percentage price change from the previous close.
    pub change_pct: f64,
    /// Absolute price change from the previous close.
    pub change_amount: Decimal,
    /// Turnover ratio as a percentage `volume / free_float * 100`.
    pub turnover_pct: f64,
}

/// A single day's capital flow data broken down by order size.
///
/// Tracks net inflows and outflows for different investor categories
/// (small, medium, large, super-large) along with their percentage
/// shares of total flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapitalFlowPoint {
    /// Trading date in `YYYY-MM-DD` format.
    pub trade_date: String,
    /// Net capital inflow for the "main" (institutional) category.
    pub main_net_inflow: Decimal,
    /// Net capital inflow for small orders.
    pub small_net_inflow: Decimal,
    /// Net capital inflow for medium orders.
    pub medium_net_inflow: Decimal,
    /// Net capital inflow for large orders.
    pub large_net_inflow: Decimal,
    /// Net capital inflow for super-large orders.
    pub super_large_net_inflow: Decimal,
    /// Main net inflow as a percentage of total flow.
    pub main_net_inflow_ratio_pct: f64,
    /// Small net inflow as a percentage of total flow.
    pub small_net_inflow_ratio_pct: f64,
    /// Medium net inflow as a percentage of total flow.
    pub medium_net_inflow_ratio_pct: f64,
    /// Large net inflow as a percentage of total flow.
    pub large_net_inflow_ratio_pct: f64,
    /// Super-large net inflow as a percentage of total flow.
    pub super_large_net_inflow_ratio_pct: f64,
    /// Closing price for the trading day.
    pub close: Decimal,
    /// Percentage price change from the previous close.
    pub change_pct: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn market_kind_debug() {
        assert_eq!(format!("{:?}", MarketKind::AShare), "AShare");
        assert_eq!(format!("{:?}", MarketKind::HongKong), "HongKong");
        assert_eq!(format!("{:?}", MarketKind::UsEquity), "UsEquity");
    }

    #[test]
    fn market_kind_clone_copy() {
        let m = MarketKind::AShare;
        let m2 = m;
        assert_eq!(m, m2);
    }

    #[test]
    fn market_kind_eq() {
        assert_eq!(MarketKind::AShare, MarketKind::AShare);
        assert_ne!(MarketKind::AShare, MarketKind::UsEquity);
    }

    #[test]
    fn quote_snapshot_serialization_roundtrip() {
        let q = QuoteSnapshot {
            symbol: "AAPL".into(),
            date: "2025-01-15".into(),
            open: dec!(150.0),
            high: dec!(155.0),
            low: dec!(149.0),
            close: dec!(153.0),
            volume: 1_000_000,
        };
        let json = serde_json::to_string(&q).unwrap();
        let q2: QuoteSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(q.symbol, q2.symbol);
        assert_eq!(q.close, q2.close);
        assert_eq!(q.volume, q2.volume);
    }

    #[test]
    fn fundamentals_snapshot_optional_fields() {
        let f = FundamentalsSnapshot {
            symbol: "TEST".into(),
            company_name: "Test Corp".into(),
            cik: "0001234567".into(),
            industry: Some("Technology".into()),
            currency: "USD".into(),
            fiscal_year_end: Some("12-31".into()),
            shares_outstanding: Some(1_000_000),
            market_cap: Some(dec!(100_000_000)),
            net_income_usd: Some(dec!(10_000_000)),
            revenues_usd: Some(dec!(50_000_000)),
            assets_usd: Some(dec!(200_000_000)),
            liabilities_usd: Some(dec!(80_000_000)),
            stockholders_equity_usd: Some(dec!(120_000_000)),
            cash_and_equivalents_usd: Some(dec!(30_000_000)),
            gross_profit_usd: Some(dec!(25_000_000)),
            operating_income_usd: Some(dec!(15_000_000)),
            operating_expenses_usd: Some(dec!(10_000_000)),
            operating_cash_flow_usd: Some(dec!(12_000_000)),
            capital_expenditure_usd: Some(dec!(2_000_000)),
            free_cash_flow_usd: Some(dec!(10_000_000)),
            long_term_debt_usd: Some(dec!(50_000_000)),
            current_debt_usd: Some(dec!(10_000_000)),
            total_debt_usd: Some(dec!(60_000_000)),
            diluted_shares_outstanding: Some(1_100_000),
        };
        assert_eq!(f.industry.as_deref(), Some("Technology"));
        assert_eq!(f.shares_outstanding, Some(1_000_000));
    }

    #[test]
    fn news_item_serialization() {
        let n = NewsItem {
            published_at: "2025-01-15T10:00:00Z".into(),
            title: "Test News".into(),
            summary: "A test article".into(),
            source: "Reuters".into(),
            url: Some("https://example.com".into()),
        };
        let json = serde_json::to_string(&n).unwrap();
        let n2: NewsItem = serde_json::from_str(&json).unwrap();
        assert_eq!(n.title, n2.title);
        assert_eq!(n.url, n2.url);
    }

    #[test]
    fn news_item_no_url() {
        let n = NewsItem {
            published_at: "2025-01-15T10:00:00Z".into(),
            title: "Test".into(),
            summary: "Summary".into(),
            source: "Source".into(),
            url: None,
        };
        let json = serde_json::to_string(&n).unwrap();
        let n2: NewsItem = serde_json::from_str(&json).unwrap();
        assert!(n2.url.is_none());
    }

    #[test]
    fn news_fetch_attempt_default() {
        let a = NewsFetchAttempt::default();
        assert!(!a.success);
        assert_eq!(a.item_count, 0);
        assert!(a.error.is_none());
        assert!(a.query.is_none());
    }

    #[test]
    fn news_fetch_attempt_with_error() {
        let a = NewsFetchAttempt {
            source: "test".into(),
            query: Some("AAPL".into()),
            success: false,
            item_count: 0,
            error: Some("timeout".into()),
        };
        let json = serde_json::to_string(&a).unwrap();
        let a2: NewsFetchAttempt = serde_json::from_str(&json).unwrap();
        assert!(!a2.success);
        assert_eq!(a2.error.as_deref(), Some("timeout"));
    }

    #[test]
    fn news_fetch_result_default() {
        let r = NewsFetchResult::default();
        assert!(r.items.is_empty());
        assert!(r.attempts.is_empty());
        assert!(!r.cacheable);
    }

    #[test]
    fn news_fetch_result_with_items() {
        let r = NewsFetchResult {
            items: vec![NewsItem {
                published_at: "2025-01-15".into(),
                title: "Title".into(),
                summary: "Summary".into(),
                source: "Source".into(),
                url: None,
            }],
            attempts: vec![],
            cacheable: true,
        };
        let json = serde_json::to_string(&r).unwrap();
        let r2: NewsFetchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r2.items.len(), 1);
        assert!(r2.cacheable);
    }

    #[test]
    fn candle_point_serialization() {
        let c = CandlePoint {
            trade_date: "2025-01-15".into(),
            open: dec!(100.0),
            close: dec!(105.0),
            high: dec!(106.0),
            low: dec!(99.0),
            volume: 500_000,
            amount: dec!(50000000.0),
            amplitude_pct: 7.0,
            change_pct: 5.0,
            change_amount: dec!(5.0),
            turnover_pct: 2.5,
        };
        let json = serde_json::to_string(&c).unwrap();
        let c2: CandlePoint = serde_json::from_str(&json).unwrap();
        assert_eq!(c.trade_date, c2.trade_date);
        assert_eq!(c.close, c2.close);
        assert!((c.change_pct - c2.change_pct).abs() < f64::EPSILON);
    }

    #[test]
    fn capital_flow_point_serialization() {
        let cf = CapitalFlowPoint {
            trade_date: "2025-01-15".into(),
            main_net_inflow: dec!(1000000.0),
            small_net_inflow: dec!(-200000.0),
            medium_net_inflow: dec!(300000.0),
            large_net_inflow: dec!(500000.0),
            super_large_net_inflow: dec!(400000.0),
            main_net_inflow_ratio_pct: 50.0,
            small_net_inflow_ratio_pct: -10.0,
            medium_net_inflow_ratio_pct: 15.0,
            large_net_inflow_ratio_pct: 25.0,
            super_large_net_inflow_ratio_pct: 20.0,
            close: dec!(105.0),
            change_pct: 5.0,
        };
        let json = serde_json::to_string(&cf).unwrap();
        let cf2: CapitalFlowPoint = serde_json::from_str(&json).unwrap();
        assert_eq!(cf.trade_date, cf2.trade_date);
        assert_eq!(cf.main_net_inflow, cf2.main_net_inflow);
        assert_eq!(cf.close, cf2.close);
    }

    #[test]
    fn capital_flow_point_negative_flows() {
        let cf = CapitalFlowPoint {
            trade_date: "2025-01-15".into(),
            main_net_inflow: dec!(-500000.0),
            small_net_inflow: dec!(-100000.0),
            medium_net_inflow: dec!(-50000.0),
            large_net_inflow: dec!(-200000.0),
            super_large_net_inflow: dec!(-150000.0),
            main_net_inflow_ratio_pct: 50.0,
            small_net_inflow_ratio_pct: 10.0,
            medium_net_inflow_ratio_pct: 5.0,
            large_net_inflow_ratio_pct: 20.0,
            super_large_net_inflow_ratio_pct: 15.0,
            close: dec!(100.0),
            change_pct: -2.0,
        };
        let json = serde_json::to_string(&cf).unwrap();
        let cf2: CapitalFlowPoint = serde_json::from_str(&json).unwrap();
        assert!(cf2.main_net_inflow < Decimal::ZERO);
    }
}

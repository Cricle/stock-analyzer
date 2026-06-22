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
/// Captures the essential OHLCV (open-high-low-close-volume) data for a
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

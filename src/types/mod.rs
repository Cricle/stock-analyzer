//! Market data primitive types for stock analysis.
//!
//! This crate provides the core data structures used throughout the stock-analyzer
//! ecosystem for representing market quotes, fundamentals, news, candlestick data,
//! and capital flow information.

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

impl From<akshare::types::MarketKind> for MarketKind {
    fn from(m: akshare::types::MarketKind) -> Self {
        match m {
            akshare::types::MarketKind::AShare => Self::AShare,
            akshare::types::MarketKind::HongKong => Self::HongKong,
            akshare::types::MarketKind::UsEquity => Self::UsEquity,
        }
    }
}

impl MarketKind {
    /// Parse a market identifier string (Chinese or English) into `MarketKind`.
    ///
    /// Accepts all common variants: "A股", "a-share", "cn", "港股", "hk", "美股", "us", etc.
    /// Defaults to `UsEquity` for unrecognized values.
    pub fn from_market_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "a股" | "a" | "a_share" | "a-share" | "ashare" | "cn" | "cn_stock" | "cn-stock"
            | "china" => Self::AShare,
            "港股" | "hk" | "hk_equity" | "hk-equity" | "hkex" | "hongkong" | "hong_kong" => {
                Self::HongKong
            }
            "美股" | "us" | "us_equity" | "us-equity" | "usa" | "us-stock" => Self::UsEquity,
            _ => Self::UsEquity,
        }
    }

    /// Stable key for cache/storage: "a_share", "hk_equity", "us_equity".
    pub fn market_key(&self) -> &'static str {
        match self {
            Self::AShare => "a_share",
            Self::HongKong => "hk_equity",
            Self::UsEquity => "us_equity",
        }
    }

    /// Market label: language-neutral English identifier.
    pub fn label(&self) -> &'static str {
        self.display_label()
    }

    /// English display label: "A-share", "HK", "US".
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::AShare => "A-share",
            Self::HongKong => "HK",
            Self::UsEquity => "US",
        }
    }

    /// Market label for Eastmoney search API filtering.
    /// Must match the values returned by `classify_search_market` in akshare-rs.
    pub fn search_market_label(&self) -> &'static str {
        match self {
            Self::AShare => "A股",
            Self::HongKong => "港股",
            Self::UsEquity => "美股",
        }
    }

    /// Exchange code: "CN", "HK", "US".
    pub fn exchange_code(&self) -> &'static str {
        match self {
            Self::AShare => "CN",
            Self::HongKong => "HK",
            Self::UsEquity => "US",
        }
    }


}

// Re-export market data types from akshare (f64-based, no Decimal wrapper).
pub use akshare::types::{
    CandlePoint, CapitalFlowPoint, FundamentalsSnapshot, NewsItem, QuoteSnapshot,
};

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

#[cfg(test)]
#[cfg(test)]
mod tests;

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

pub use akshare::stock::feature::{
    EarningsForecast, FundFlowEntry, HotStockXq, LhbStockStatistic, MarginRatioPa, ZtPool,
};


mod a_share;
mod akshare_rust;
mod cache;
mod client;
pub mod diagnosis;
mod hk;
pub(crate) mod news_filter;
mod us;
mod wire;

pub use cache::{Singleflight, SingleflightGuard, SingleflightResult};

/// Configuration for constructing a `MarketDataClient`.
/// The backend builds this from its `Settings`.
pub struct DataConfig {
}

const MARKET_DATA_CACHE_PREFIX: &str = "stockanalyzer:marketdata";
const QUOTE_CACHE_VERSION: &str = "v5";
const FUNDAMENTALS_CACHE_VERSION: &str = "v6";
const CANDLES_CACHE_VERSION: &str = "v5";
const NEWS_CACHE_VERSION: &str = "v5";
const GLOBAL_NEWS_CACHE_VERSION: &str = "v2";
const QUOTE_CACHE_TTL_SECS: u64 = 120;
const FUNDAMENTALS_CACHE_TTL_SECS: u64 = 6 * 60 * 60;
const NEWS_CACHE_TTL_SECS: u64 = 10 * 60;
const GLOBAL_NEWS_CACHE_TTL_SECS: u64 = 10 * 60;
const INSIDER_CACHE_TTL_SECS: u64 = 15 * 60;
const CANDLES_CACHE_TTL_SECS: u64 = 5 * 60;
#[derive(Clone)]
pub struct MarketDataClient {
    ak: akshare::AkShareClient,
    pub(crate) singleflight: Singleflight,
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

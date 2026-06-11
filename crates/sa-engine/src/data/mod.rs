use std::fmt;

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use serde::{Deserialize, Serialize};

pub use crate::types::{
    CandlePoint, CapitalFlowPoint, FundamentalsSnapshot, MarketKind, NewsFetchAttempt,
    NewsFetchResult, NewsItem, QuoteSnapshot,
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
    AnalystDetail, AnalystRank, BalanceSheet, CashFlowSheet, CommentDesireIndex,
    CommentFocusIndex, CommentHistScore, CommentOrgParticipation, DividendInfo, DzjyHygtj,
    DzjyHyyybtj, DzjyMrtj, DzjyYybph, EarningsForecast, EarningsQuickReport, EarningsReport,
    EsgRating, FundFlowEntry, GdfxHoldingAnalyse, GdfxHoldingChange, GdfxHoldingDetail,
    GdfxHoldingStatistic, GdfxTeamwork, GdfxTop10, Gdhs, GdhsDetail, Ggcg, GpzyDistributeEntry,
    GpzyIndustry, GpzyPledgeDetail, GpzyPledgeRatio, GpzyPledgeRatioDetail, GpzyProfile,
    HotStockXq, IndustryCategory, JgdyDetail, JgdyTj, LhbDetail, LhbHyyyb, LhbJgmmtj,
    LhbJgstatistic, LhbStockDetail, LhbStockDetailDate, LhbStockStatistic, LhbTraderStatistic,
    LhbYybDetail, LhbYybph, MainFundFlow, MarginAccountInfo, MarginRatioPa, MarginSseDetail,
    MarginSseSummary, MarginSzseDetail, MarginSzseSummary, PankouChange, ProfitSheet,
    SectorFundFlowRank, StockComment, ZtPool, ZtPoolDtgc, ZtPoolPrevious, ZtPoolStrong,
    ZtPoolSubNew, ZtPoolZbgc,
};

// HK-specific types
pub use akshare::stock::hk_extra::{
    HkFamousStock, HkFhpxDetailThs, HkGxlLg, HkHotRank, HkHotRankDetail, HkSpotQuote,
    HkValuationBaidu,
};

// US-specific types
pub use akshare::stock::us_extra::{UsFamousStock, UsPinkStock, UsSpotSina, UsValuationBaidu};

// Xueqiu (shared HK/US)
pub use akshare::stock::xueqiu::XqStockSpot;

mod a_share;
mod akshare_rust;
mod cache;
mod client;
pub mod diagnosis;
mod hk;
mod news_search;
pub mod qdrant;
pub mod search;
mod tushare;
mod us;
mod wire;

pub use cache::{Singleflight, SingleflightGuard, SingleflightResult};

/// Configuration for constructing a `MarketDataClient`.
/// The backend builds this from its `Settings`.
pub struct DataConfig {
    pub tushare_token: Option<String>,
    pub redis_url: Option<String>,
    pub search_providers: Vec<SearchProviderConfig>,
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
const SEARXNG_QUERY_CACHE_TTL_SECS: u64 = 5 * 60;
const SEARXNG_QUERY_NEGATIVE_CACHE_TTL_SECS: u64 = 30;
const UAPIS_QUERY_CACHE_TTL_SECS: u64 = 15 * 60;
const UAPIS_QUERY_NEGATIVE_CACHE_TTL_SECS: u64 = 2 * 60;
const INSIDER_CACHE_TTL_SECS: u64 = 15 * 60;
const CANDLES_CACHE_TTL_SECS: u64 = 5 * 60;
const SEARCH_CACHE_TTL_SECS: u64 = 60 * 60;
const SEARCH_CACHE_VERSION: &str = "v4";
const HK_SECURITIES_LIST_CACHE_TTL_SECS: u64 = 24 * 60 * 60;
#[cfg(feature = "redis-cache")]
const STALE_CACHE_TTL_MULTIPLIER: u64 = 12;
#[cfg(feature = "redis-cache")]
const CACHE_TTL_JITTER_PCT: u64 = 15;

#[derive(Clone)]
pub struct MarketDataClient {
    http: reqwest_middleware::ClientWithMiddleware,
    tushare_token: Option<String>,
    #[cfg(feature = "redis-cache")]
    redis: Option<redis::aio::ConnectionManager>,
    search_providers: Vec<SearchProviderConfig>,
    ak: akshare::AkShareClient,
    pub(crate) singleflight: Singleflight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchProviderKind {
    Searxng,
    Gdelt,
    Baidu,
    Uapis,
}

#[derive(Debug, Clone)]
pub struct SearchProviderConfig {
    kind: SearchProviderKind,
    name: String,
    base_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchScope {
    News,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralSearchIntent {
    CompanyEvidence,
    MacroEvidence,
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

// Shared types (QuoteSnapshot, FundamentalsSnapshot, NewsItem, etc.)
// are re-exported from crate::types via data/mod.rs.

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SearxngNewsQueryCacheEntry {
    #[serde(default)]
    items: Vec<NewsItem>,
    #[serde(default)]
    cached_error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SearxngNewsEvidenceCacheEntry {
    #[serde(default)]
    items: Vec<NewsItem>,
}

impl SearchProviderConfig {
    fn searxng(name: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            kind: SearchProviderKind::Searxng,
            name: name.into(),
            base_url: base_url.into(),
        }
    }

    fn gdelt(name: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            kind: SearchProviderKind::Gdelt,
            name: name.into(),
            base_url: base_url.into(),
        }
    }

    fn baidu(name: impl Into<String>) -> Self {
        Self {
            kind: SearchProviderKind::Baidu,
            name: name.into(),
            base_url: String::new(),
        }
    }

    fn uapis(name: impl Into<String>) -> Self {
        Self {
            kind: SearchProviderKind::Uapis,
            name: name.into(),
            base_url: String::new(),
        }
    }

    fn cache_scope(&self) -> String {
        format!(
            "{}:{}",
            self.name.trim().to_ascii_lowercase(),
            self.base_url.trim().trim_end_matches('/')
        )
    }

    fn display_name(&self) -> String {
        match self.kind {
            SearchProviderKind::Searxng => {
                if self.name.eq_ignore_ascii_case("searxng") {
                    "SearXNG News".to_string()
                } else {
                    format!("{} News", self.name)
                }
            }
            SearchProviderKind::Gdelt => {
                if self.name.eq_ignore_ascii_case("gdelt") {
                    "GDELT News".to_string()
                } else {
                    format!("{} News", self.name)
                }
            }
            SearchProviderKind::Baidu => "Baidu News".to_string(),
            SearchProviderKind::Uapis => "Uapis News".to_string(),
        }
    }

    fn supports_scope(&self, scope: SearchScope) -> bool {
        match self.kind {
            SearchProviderKind::Searxng => true,
            SearchProviderKind::Gdelt => scope == SearchScope::News,
            SearchProviderKind::Baidu => scope == SearchScope::News,
            SearchProviderKind::Uapis => true,
        }
    }

    fn query_budget(&self, scope: SearchScope) -> usize {
        match self.kind {
            SearchProviderKind::Searxng => usize::MAX,
            SearchProviderKind::Gdelt => match scope {
                SearchScope::News => 2,
                SearchScope::General => 0,
            },
            SearchProviderKind::Baidu => match scope {
                SearchScope::News => 3,
                SearchScope::General => 0,
            },
            SearchProviderKind::Uapis => usize::MAX,
        }
    }

    fn rewrite_query(&self, query: &str, language: &str) -> String {
        match self.kind {
            SearchProviderKind::Searxng => query.trim().to_string(),
            SearchProviderKind::Gdelt => rewrite_query_for_gdelt(query, language),
            SearchProviderKind::Baidu => query.trim().to_string(),
            SearchProviderKind::Uapis => query.trim().to_string(),
        }
    }

    fn cache_ttl_secs(&self) -> u64 {
        match self.kind {
            SearchProviderKind::Uapis => UAPIS_QUERY_CACHE_TTL_SECS,
            _ => SEARXNG_QUERY_CACHE_TTL_SECS,
        }
    }

    fn negative_cache_ttl_secs(&self) -> u64 {
        match self.kind {
            SearchProviderKind::Uapis => UAPIS_QUERY_NEGATIVE_CACHE_TTL_SECS,
            _ => SEARXNG_QUERY_NEGATIVE_CACHE_TTL_SECS,
        }
    }
}

impl SearchScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::News => "news",
            Self::General => "general",
        }
    }
}

const GENERAL_SEARCH_FALLBACK_QUERY_LIMIT: usize = 6;
const NEWS_SEARCH_PROVIDER_TIMEOUT_SECS: u64 = 6;
const NEWS_SEARCH_EVIDENCE_QUERY_LIMIT_PER_PROVIDER: usize = 2;

pub(crate) fn news_result_cacheable(items: &[NewsItem], attempts: &[NewsFetchAttempt]) -> bool {
    !items.is_empty() && !attempts.is_empty() && attempts.iter().all(|attempt| attempt.success)
}

// CandlePoint and CapitalFlowPoint are re-exported from crate::types via data/mod.rs.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorSnapshot {
    pub sector_code: String,
    pub sector_name: String,
    pub latest_index: f64,
    pub change_pct: f64,
    pub main_net_inflow: f64,
    pub main_net_inflow_ratio_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorConstituent {
    pub symbol: String,
    pub name: String,
    pub latest_price: f64,
    pub change_pct: f64,
    pub main_net_inflow: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSearchResult {
    pub symbol: String,
    pub name: String,
    pub market: String,
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HkSecurityDirectoryEntry {
    symbol: String,
    name: String,
    market: String,
    exchange: String,
    category: String,
    sub_category: String,
    trading_currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsSecurityDirectoryEntry {
    symbol: String,
    name: String,
    market: String,
    exchange: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncementDetail {
    pub art_code: String,
    pub title: String,
    pub published_at: String,
    pub content: String,
    pub pdf_url: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncementItem {
    pub art_code: String,
    pub symbol: String,
    pub title: String,
    pub published_at: String,
    pub url: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillboardEntry {
    pub trade_date: String,
    pub symbol: String,
    pub name: String,
    pub close_price: f64,
    pub change_rate_pct: f64,
    pub turnover_rate_pct: Option<f64>,
    pub net_amount: Option<f64>,
    pub buy_amount: Option<f64>,
    pub sell_amount: Option<f64>,
    pub explanation: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillboardSeatDetail {
    pub trade_date: String,
    pub symbol: String,
    pub department_name: String,
    pub buy_amount: Option<f64>,
    pub sell_amount: Option<f64>,
    pub net_amount: Option<f64>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeCalendarItem {
    pub exchange: String,
    pub calendar_date: String,
    pub is_open: bool,
    pub previous_trade_date: Option<String>,
}

fn rewrite_query_for_gdelt(query: &str, language: &str) -> String {
    let mut tokens = query
        .split_whitespace()
        .filter(|token| {
            let normalized =
                token.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == '(' || ch == ')');
            !normalized.is_empty()
                && !normalized.eq_ignore_ascii_case("or")
                && !normalized.eq_ignore_ascii_case("and")
                && !normalized.eq_ignore_ascii_case("not")
                && !normalized.starts_with("site:")
        })
        .map(|token| {
            token.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == '(' || ch == ')')
        })
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    if language.eq_ignore_ascii_case("en-US") && tokens.len() > 6 {
        tokens.truncate(6);
    }
    if language.eq_ignore_ascii_case("zh-CN") && tokens.len() > 8 {
        tokens.truncate(8);
    }
    tokens.join(" ")
}


pub mod news_filter;

pub use news_filter::*;

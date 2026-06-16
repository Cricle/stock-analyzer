use std::fmt;

use crate::types::NewsFetchAttempt;
pub use crate::types::{
    CandlePoint, CapitalFlowPoint, FundamentalsSnapshot, MarketKind, NewsItem, QuoteSnapshot,
};

// ---------------------------------------------------------------------------
// Conversion from akshare-rs types to our domain types.
// ---------------------------------------------------------------------------

pub(crate) fn news_item_from_stock_news(n: akshare::stock::feature::StockNews) -> NewsItem {
    NewsItem {
        published_at: n.publish_time,
        title: n.title,
        summary: n.content.unwrap_or_default(),
        source: n.source.unwrap_or_else(|| "Eastmoney".to_string()),
        url: n.url,
    }
}

pub(crate) fn news_item_from_news_entry_with_source(
    n: akshare::stock::feature::NewsEntry,
    source: &str,
) -> NewsItem {
    NewsItem {
        published_at: n.time,
        title: n.title,
        summary: n.summary.unwrap_or_default(),
        source: source.to_string(),
        url: n.url,
    }
}

pub(crate) fn news_item_from_announcement(a: akshare::AnnouncementItem) -> NewsItem {
    NewsItem {
        published_at: a.published_at,
        title: a.title,
        summary: String::new(),
        source: a.source,
        url: a.url,
    }
}

pub use akshare::stock::feature::{
    EarningsForecast, FundFlowEntry, HotStockXq, LhbStockStatistic, MarginRatioPa, ZtPool,
};

mod a_share;
mod client;
pub mod diagnosis;
mod hk;
pub(crate) mod news;
mod us;

// ---------------------------------------------------------------------------
// API Key Pool (environment variables + config file)
// ---------------------------------------------------------------------------

#[derive(Clone, Default, serde::Deserialize)]
struct ConfigFile {
    api_keys: Option<ConfigApiKeys>,
}

#[derive(Clone, Default, serde::Deserialize)]
struct ConfigApiKeys {
    finnhub: Option<Vec<String>>,
}

#[derive(Clone, Default)]
pub(crate) struct ApiKeyPool {
    finnhub_keys: Vec<String>,
}

impl ApiKeyPool {
    pub fn load() -> Self {
        // 1. Try config file
        let config_path = std::env::var("SA_ENGINE_CONFIG").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/.config/sa-engine/config.toml")
        });
        let file_config: ConfigFile = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();

        // 2. Env vars override config file
        let finnhub_keys = std::env::var("FINNHUB_API_KEY")
            .ok()
            .map(|v| Self::parse_keys(&v))
            .filter(|v| !v.is_empty())
            .or(file_config.api_keys.as_ref().and_then(|c| c.finnhub.clone()))
            .unwrap_or_default();

        if !finnhub_keys.is_empty() {
            tracing::info!(
                finnhub_keys = finnhub_keys.len(),
                "API key pool loaded"
            );
        }
        Self { finnhub_keys }
    }

    fn parse_keys(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect()
    }

    pub fn next_finnhub_key(&self) -> Option<&str> {
        if self.finnhub_keys.is_empty() {
            return None;
        }
        let idx = (chrono::Utc::now().timestamp() / 60) as usize % self.finnhub_keys.len();
        Some(&self.finnhub_keys[idx])
    }
}

// ---------------------------------------------------------------------------
// MarketDataClient
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MarketDataClient {
    ak: akshare::AkShareClient,
    pub(crate) api_keys: ApiKeyPool,
}

#[derive(Debug)]
pub struct DataError {
    message: String,
}

impl DataError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
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

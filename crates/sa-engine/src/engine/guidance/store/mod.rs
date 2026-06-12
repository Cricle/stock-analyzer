//! Trait-based storage for daily guidance reports.
//!
//! Uses `crate::models::CacheStore` for caching.

mod cache;

use super::DailyGuidanceReport;

const GUIDANCE_CACHE_PREFIX: &str = "tradingagents:guidance";
const GUIDANCE_CACHE_TTL_SECS: u64 = 4 * 60 * 60; // 4 hours
const GUIDANCE_STALE_TTL_SECS: u64 = 24 * 60 * 60; // 24 hours

/// Pre-fetched data for two-phase report generation.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PreparedData {
    pub market: String,
    pub date: String,
    pub news_json: String,
    pub news_sources: Vec<String>,
    pub historical_insights_json: String,
    pub market_indices_json: String,
    pub recent_stock_picks_json: Option<String>,
    pub prepared_at: String,
}

#[derive(Clone)]
pub struct GuidanceStore {
    cache: std::sync::Arc<dyn crate::models::CacheStore>,
}

impl GuidanceStore {
    pub fn new(cache: std::sync::Arc<dyn crate::models::CacheStore>) -> Self {
        Self { cache }
    }

    /// Create from environment variables (legacy compatibility).
    pub fn from_env() -> Self {
        tracing::warn!("GuidanceStore::from_env() called without injected stores; using no-op fallback");
        Self {
            cache: std::sync::Arc::new(NoopCacheStore),
        }
    }
}

/// Returns an appropriate cache TTL based on market trading hours.
///
/// During market hours, uses the base TTL. After hours, extends to 12x the base
/// since data changes less frequently when markets are closed.
pub fn market_ttl(market: &str, base_ttl: std::time::Duration) -> std::time::Duration {
    use chrono::{Timelike, Utc};

    let now = Utc::now();
    let hour = now.hour();
    let minute = now.minute();
    let minutes_of_day = hour * 60 + minute;

    let is_market_hours = match market {
        "a_share" | "hong_kong" => (75..420).contains(&minutes_of_day),
        "us_equity" => (870..1260).contains(&minutes_of_day),
        _ => false,
    };

    if is_market_hours {
        base_ttl
    } else {
        base_ttl * 12
    }
}

// No-op implementations for from_env() fallback

struct NoopCacheStore;

#[async_trait::async_trait]
impl crate::models::CacheStore for NoopCacheStore {
    async fn get(&self, _key: &str) -> anyhow::Result<Option<Vec<u8>>> { Ok(None) }
    async fn set(&self, _key: &str, _value: &[u8], _ttl_seconds: Option<u64>) -> anyhow::Result<()> { Ok(()) }
    async fn delete(&self, _key: &str) -> anyhow::Result<()> { Ok(()) }
    async fn exists(&self, _key: &str) -> anyhow::Result<bool> { Ok(false) }
    async fn list_entries(&self, _prefix: &str) -> anyhow::Result<Vec<crate::models::CacheEntry>> { Ok(vec![]) }
}

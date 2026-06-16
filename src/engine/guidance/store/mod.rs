//! Trait-based storage for daily guidance reports.
//!
//! Uses `crate::models::CacheStore` for caching.

mod cache;

use super::DailyGuidanceReport;

const GUIDANCE_CACHE_PREFIX: &str = "tradingagents:guidance";
const GUIDANCE_CACHE_TTL_SECS: u64 = 4 * 60 * 60; // 4 hours
const GUIDANCE_STALE_TTL_SECS: u64 = 24 * 60 * 60; // 24 hours


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

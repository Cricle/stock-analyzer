//! sa-data — re-exports from akshare-rs for stock-analyzer engine.
//!
//! All data fetching is delegated to akshare-rs. This crate only re-exports
//! the types and client the engine needs.

pub mod cache;
pub mod pipeline;
pub mod validator;

// Re-export MarketDataClient and config types from akshare-rs
pub use akshare::provider::market_client::{GeneralSearchIntent, MarketDataClient};

// Re-export DataConfig from akshare for backward compatibility
pub use akshare::provider::market_client::DataConfig;

// Re-export data types from akshare-rs
pub use akshare::types::{
    BillboardEntry, CandlePoint, CapitalFlowPoint, FundamentalsSnapshot, MarketKind, NewsItem,
    QuoteSnapshot,
};

// Re-export stock feature types from akshare-rs
pub use akshare::stock::feature::{
    BillboardStockStatistic, EarningsForecast, FundFlowEntry, HotStockXq, MarginRatioPa, ZtPool,
};

// Re-export DataFetchDiagnosis from akshare-rs
pub use akshare::provider::market_client::DataFetchDiagnosis;

// Re-export news filter utilities
pub use akshare::provider::market_client::normalized_news_date;

/// Stub for Redis-backed cache store (not available in this version).
/// Accepts any type to avoid requiring the redis crate as a dependency.
pub struct RedisCacheStore;

impl RedisCacheStore {
    /// Create a stub Redis cache store (no-op in this version).
    pub fn new<T>(_conn: T) -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl crate::CacheStore for RedisCacheStore {
    async fn get(&self, _key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn set(
        &self,
        _key: &str,
        _value: &[u8],
        _ttl_seconds: Option<u64>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn delete(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn exists(&self, _key: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
    async fn list_entries(&self, _prefix: &str) -> anyhow::Result<Vec<crate::CacheEntry>> {
        Ok(vec![])
    }
}

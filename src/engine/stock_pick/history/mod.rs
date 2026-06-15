//! Stock pick history storage using trait-based CacheStore.

use serde::{Deserialize, Serialize};

use crate::models::{
    StockPickEvidenceCoverageSummary, StockPickHistoryMatchSnapshot,
    StockPickResponse, StockPickStorageWriteSummary,
};

const STOCK_PICK_CACHE_PREFIX: &str = "tradingagents:stock_pick";
const STOCK_PICK_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Clone)]
pub(crate) struct StockPickHistoryStore {
    cache: std::sync::Arc<dyn crate::models::CacheStore>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StockPickEvidencePayload {
    pub symbol: String,
    pub market: String,
    pub theme_key: String,
    pub analysis_date: String,
    pub query: String,
    pub published_at: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub url: String,
    pub evidence_type: String,
    pub sentiment_hint: String,
    pub hard_negative_flag: bool,
    pub dedupe_key: String,
}

impl StockPickHistoryStore {
    /// Create from environment variables (legacy compatibility).
    pub(crate) fn from_env() -> anyhow::Result<Self> {
        tracing::warn!("StockPickHistoryStore::from_env() called without injected stores; using no-op fallback");
        Ok(Self {
            cache: std::sync::Arc::new(NoopCacheStore),
        })
    }

    pub(crate) async fn write_run(
        &self,
        run_id: &str,
        request_market: &str,
        response: &StockPickResponse,
        _theme_keys: &[(String, String)],
        _evidence_payloads: &[StockPickEvidencePayload],
    ) -> anyhow::Result<StockPickStorageWriteSummary> {
        let mut cache_keys_written = 0usize;

        // Write run data to cache
        let run_key = format!("{STOCK_PICK_CACHE_PREFIX}:run:{run_id}");
        let run_payload = serde_json::to_vec(response)?;
        self.cache.set(&run_key, &run_payload, Some(STOCK_PICK_CACHE_TTL_SECS)).await?;
        cache_keys_written += 1;

        // Write summary to cache
        let summary_key = format!(
            "{STOCK_PICK_CACHE_PREFIX}:summary:{}:{}:{}",
            request_market.trim().to_ascii_lowercase(),
            response.analysis_date.trim(),
            response.strategy.trim().to_ascii_lowercase().replace(' ', "_")
        );
        let summary_payload = serde_json::to_vec(&response.summary)?;
        self.cache.set(&summary_key, &summary_payload, Some(STOCK_PICK_CACHE_TTL_SECS)).await?;
        cache_keys_written += 1;

        // Vector store writes removed with RAG system.
        Ok(StockPickStorageWriteSummary {
            redis_keys_written: cache_keys_written,
            vector_points_written: 0,
        })
    }

    pub(crate) async fn read_history(
        &self,
        _symbol: &str,
        _market: &str,
        _theme_key: &str,
        _current_price: Option<f64>,
    ) -> anyhow::Result<StockPickHistoryMatchSnapshot> {
        // Vector-based history search removed with RAG system.
        Ok(StockPickHistoryMatchSnapshot {
            enabled: true,
            sample_count: 0,
            vector_hit_count: 0,
            average_score: None,
            hit_rate: None,
            average_alpha_return: None,
            top_matches: Vec::new(),
        })
    }

    pub(crate) fn build_evidence_coverage_summary(
        &self,
        light_search_symbols: usize,
        deep_search_symbols: usize,
        evidence_records_indexed: usize,
        history_records_matched: usize,
    ) -> StockPickEvidenceCoverageSummary {
        StockPickEvidenceCoverageSummary {
            light_search_symbols,
            deep_search_symbols,
            evidence_records_indexed,
            history_records_matched,
        }
    }
}

// No-op fallback implementation

struct NoopCacheStore;

#[async_trait::async_trait]
impl crate::models::CacheStore for NoopCacheStore {
    async fn get(&self, _key: &str) -> anyhow::Result<Option<Vec<u8>>> { Ok(None) }
    async fn set(&self, _key: &str, _value: &[u8], _ttl_seconds: Option<u64>) -> anyhow::Result<()> { Ok(()) }
    async fn delete(&self, _key: &str) -> anyhow::Result<()> { Ok(()) }
    async fn exists(&self, _key: &str) -> anyhow::Result<bool> { Ok(false) }
    async fn list_entries(&self, _prefix: &str) -> anyhow::Result<Vec<crate::models::CacheEntry>> { Ok(vec![]) }
}

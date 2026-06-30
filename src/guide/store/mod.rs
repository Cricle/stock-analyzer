//! Trait-based storage for daily guidance reports.
//!
//! Uses `crate::CacheStore` for caching and `crate::VectorStore` for vector search.

mod cache;
mod search;
mod write;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::DailyGuidanceReport;

// Re-export embedding functions from the embedding module.
// hash_embed is kept as a fallback; prefer semantic_embed for production.
pub use crate::guide::embedding::{hash_embed, semantic_embed};

const GUIDANCE_CACHE_PREFIX: &str = "tradingagents:guidance";
const GUIDANCE_CACHE_TTL_SECS: u64 = 4 * 60 * 60; // 4 hours
const GUIDANCE_STALE_TTL_SECS: u64 = 24 * 60 * 60; // 24 hours
const GUIDANCE_VECTOR_COLLECTION: &str = "tradingagents_daily_guidance";

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
    cache: std::sync::Arc<dyn crate::CacheStore>,
    vector_store: std::sync::Arc<dyn crate::VectorStore>,
}

impl GuidanceStore {
    pub fn new(
        cache: std::sync::Arc<dyn crate::CacheStore>,
        vector_store: std::sync::Arc<dyn crate::VectorStore>,
    ) -> Self {
        Self {
            cache,
            vector_store,
        }
    }

    /// Create from environment variables (legacy compatibility).
    ///
    /// TODO: Replace with explicit dependency injection.
    /// This falls back to no-op stores when no concrete implementation is available.
    pub fn from_env() -> Self {
        tracing::warn!(
            "GuidanceStore::from_env() called without injected stores; using no-op fallback"
        );
        Self {
            cache: std::sync::Arc::new(NoopCacheStore),
            vector_store: std::sync::Arc::new(NoopVectorStore),
        }
    }

    // --- Helpers ---

    pub fn vector_point_id(entry_id: &str) -> String {
        let digest = Sha256::digest(entry_id.as_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes).to_string()
    }

    pub fn news_dedup_key(title: &str, source: &str) -> String {
        let digest = Sha256::digest(format!(
            "{}:{}",
            title.trim().to_ascii_lowercase(),
            source.trim().to_ascii_lowercase()
        ));
        hex::encode(&digest[..16])
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
        "a_share" | "hong_kong" => minutes_of_day >= 75 && minutes_of_day < 420,
        "us_equity" => minutes_of_day >= 870 && minutes_of_day < 1260,
        _ => false,
    };

    if is_market_hours {
        base_ttl
    } else {
        base_ttl * 12
    }
}

// No-op implementations for from_env() fallback
use crate::noop_stores::{NoopCacheStore, NoopVectorStore};

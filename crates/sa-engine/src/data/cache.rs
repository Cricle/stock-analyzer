use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

use super::{MARKET_DATA_CACHE_PREFIX, MarketDataClient, MarketKind};

impl MarketDataClient {
    pub(super) fn normalize_a_share_symbol(&self, symbol: &str) -> Option<String> {
        akshare::normalize_a_share_symbol(symbol)
    }

    pub(super) fn normalize_hk_symbol(&self, symbol: &str) -> Option<String> {
        akshare::normalize_hk_symbol(symbol).map(|code| format!("{code}.HK"))
    }

    pub(super) fn cache_symbol(&self, symbol: &str, market: MarketKind) -> String {
        match market {
            MarketKind::AShare => self
                .normalize_a_share_symbol(symbol)
                .unwrap_or_else(|| symbol.trim().to_uppercase()),
            MarketKind::HongKong => self
                .normalize_hk_symbol(symbol)
                .unwrap_or_else(|| symbol.trim().to_uppercase()),
            MarketKind::UsEquity => symbol.trim().to_uppercase(),
        }
    }

    // --- Cache methods (no-op stubs) ---

    pub(super) async fn cache_get_json<T>(&self, _key: &str) -> Option<T>
    where
        T: DeserializeOwned,
    {
        None
    }

    pub(super) async fn cache_get_json_exact<T>(&self, _key: &str) -> Option<T>
    where
        T: DeserializeOwned,
    {
        None
    }

    pub(super) async fn cache_set_json<T>(&self, _key: &str, _ttl_secs: u64, _value: &T)
    where
        T: Serialize,
    {
    }

    pub(super) async fn cache_mget_json<T>(&self, keys: &[String]) -> Vec<Option<T>>
    where
        T: DeserializeOwned,
    {
        keys.iter().map(|_| None).collect()
    }

    // --- Shared utility methods ---

    pub(super) fn stale_cache_key(&self, key: &str) -> String {
        format!("{key}:stale")
    }

    pub(super) fn normalized_news_query(&self, query: &str) -> String {
        query
            .trim()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub(super) fn news_query_cache_component(&self, query: Option<&str>) -> String {
        let normalized = query
            .map(|value| self.normalized_news_query(value))
            .unwrap_or_default();
        let digest = Sha256::digest(normalized.as_bytes());
        format!("{digest:x}")
    }

    pub(crate) fn normalize_optional_query(query: Option<&str>) -> Option<String> {
        query
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    }

    pub(super) fn search_query_cache_key(
        &self,
        provider: &super::SearchProviderConfig,
        query: &str,
        language: &str,
        time_range: Option<&str>,
        scope: super::SearchScope,
    ) -> String {
        let digest = Sha256::digest(
            format!(
                "provider={}|q={}|language={}|time_range={}|scope={}",
                provider.cache_scope(),
                self.normalized_news_query(query),
                language.trim(),
                time_range.unwrap_or_default().trim(),
                scope.as_str(),
            )
            .as_bytes(),
        );
        format!("{MARKET_DATA_CACHE_PREFIX}:search:news_query:v2:{digest:x}")
    }

    pub(super) fn search_evidence_cache_key(
        &self,
        queries: &[&str],
        language: &str,
        time_range: Option<&str>,
        scope: super::SearchScope,
    ) -> String {
        let mut normalized_queries = queries
            .iter()
            .map(|query| self.normalized_news_query(query))
            .filter(|query| !query.is_empty())
            .collect::<Vec<_>>();
        normalized_queries.sort();
        normalized_queries.dedup();
        let mut provider_scopes = self
            .search_providers
            .iter()
            .map(super::SearchProviderConfig::cache_scope)
            .collect::<Vec<_>>();
        provider_scopes.sort();
        provider_scopes.dedup();
        let digest = Sha256::digest(
            format!(
                "providers={}|queries={}|language={}|time_range={}|scope={}",
                provider_scopes.join("|"),
                normalized_queries.join("|"),
                language.trim(),
                time_range.unwrap_or_default().trim(),
                scope.as_str(),
            )
            .as_bytes(),
        );
        format!("{MARKET_DATA_CACHE_PREFIX}:search:news_evidence:v2:{digest:x}")
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(super) fn searxng_query_cache_key(
        &self,
        query: &str,
        language: &str,
        time_range: Option<&str>,
        scope: super::SearchScope,
    ) -> String {
        let provider = self
            .search_providers
            .iter()
            .find(|provider| provider.kind == super::SearchProviderKind::Searxng)
            .cloned()
            .unwrap_or_else(|| {
                super::SearchProviderConfig::searxng("searxng", "http://127.0.0.1:8080")
            });
        self.search_query_cache_key(&provider, query, language, time_range, scope)
    }
}

// ============================================================
// Cache Stampede Protection (Singleflight)
// ============================================================

/// Result of entering a singleflight operation.
pub enum SingleflightResult<'a> {
    /// This caller is the leader and should compute the value.
    /// The guard automatically cleans up on drop.
    Leader(SingleflightGuard<'a>),
    /// Another caller was already computing and has finished.
    /// The caller should retry its cache lookup (which should now be a hit).
    Waiting,
}

/// Guard that ensures singleflight cleanup on drop.
///
/// When dropped (including on panic), removes the in-flight entry and notifies
/// all waiting tasks to retry their cache lookup.
pub struct SingleflightGuard<'a> {
    singleflight: &'a Singleflight,
    key: String,
}

impl Drop for SingleflightGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut map) = self.singleflight.in_flight.lock()
            && let Some(notify) = map.remove(&self.key)
        {
            drop(map);
            notify.notify_waiters();
        }
    }
}

/// Prevents cache stampede by ensuring only one computation per key runs at a time.
///
/// When multiple concurrent requests for the same uncached data arrive, only the first
/// (leader) actually fetches from the upstream data source. All other requests (followers)
/// wait for the leader to finish, then retry their cache lookup, which should now be a hit.
///
/// # Example
///
/// ```ignore
/// if let Some(cached) = self.cache_get_json(&key).await {
///     return Ok(cached);
/// }
/// match singleflight.enter(&key).await {
///     SingleflightResult::Leader(_guard) => {
///         let data = fetch_from_upstream().await?;
///         self.cache_set_json(&key, ttl, &data).await;
///         // _guard dropped here, waking followers
///     }
///     SingleflightResult::Waiting => {
///         if let Some(cached) = self.cache_get_json(&key).await {
///             return Ok(cached);
///         }
///         // Leader failed; fall through to compute ourselves
///     }
/// }
/// ```
pub struct Singleflight {
    in_flight: Arc<std::sync::Mutex<HashMap<String, Arc<Notify>>>>,
}

impl Clone for Singleflight {
    fn clone(&self) -> Self {
        Self {
            in_flight: Arc::clone(&self.in_flight),
        }
    }
}

impl Default for Singleflight {
    fn default() -> Self {
        Self::new()
    }
}

impl Singleflight {
    pub fn new() -> Self {
        Self {
            in_flight: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Enter a singleflight operation for the given key.
    ///
    /// Returns [`SingleflightResult::Leader`] if this caller should compute the value.
    /// Returns [`SingleflightResult::Waiting`] if another caller was already computing
    /// and has now finished (the caller should retry its cache lookup).
    pub async fn enter(&self, key: &str) -> SingleflightResult<'_> {
        let (is_leader, notify) = {
            let mut map = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(n) = map.get(key) {
                (false, n.clone())
            } else {
                let n = Arc::new(Notify::new());
                map.insert(key.to_string(), n.clone());
                (true, n)
            }
        };

        if is_leader {
            SingleflightResult::Leader(SingleflightGuard {
                singleflight: self,
                key: key.to_string(),
            })
        } else {
            // Wait for the leader to finish
            notify.notified().await;
            SingleflightResult::Waiting
        }
    }

    /// Execute a computation with singleflight protection.
    ///
    /// The first caller for a given key becomes the leader and runs `compute`.
    /// Subsequent callers wait for the leader to finish, then run `compute` themselves.
    /// In cache scenarios, `compute` should check cache first, so followers will get
    /// fast cache hits after the leader populates the cache.
    pub async fn do_once<F, Fut, T>(&self, key: &str, compute: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let _guard = match self.enter(key).await {
            SingleflightResult::Leader(guard) => guard,
            SingleflightResult::Waiting => {
                // Leader finished; run compute which should hit cache
                return compute().await;
            }
        };
        // We're the leader
        compute().await
        // _guard dropped here, waking followers
    }
}

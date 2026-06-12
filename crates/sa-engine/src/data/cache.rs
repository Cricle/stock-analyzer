use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

use super::{MarketDataClient, MarketKind};

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

    // --- Cache no-op stubs (caching removed) ---

    pub(super) async fn cache_get_json<T>(&self, _key: &str) -> Option<T>
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
    pub async fn do_once<F, Fut, T>(&self, key: &str, compute: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let _guard = match self.enter(key).await {
            SingleflightResult::Leader(guard) => guard,
            SingleflightResult::Waiting => {
                return compute().await;
            }
        };
        compute().await
    }
}

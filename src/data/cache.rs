use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct CacheEntry {
    data: serde_json::Value,
    cached_at: Instant,
    ttl: Duration,
}

/// LRU cache for market data with TTL support.
pub struct DataCacheLayer {
    cache: Mutex<lru::LruCache<String, CacheEntry>>,
    default_ttl: Duration,
}

impl DataCacheLayer {
    /// Create a new cache with the given max size and default TTL.
    pub fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            cache: Mutex::new(lru::LruCache::new(
                NonZeroUsize::new(max_size.max(1)).unwrap(),
            )),
            default_ttl,
        }
    }

    /// Get a value from the cache. Returns None if missing or expired.
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        let mut cache = self.cache.lock().unwrap();
        let expired = cache
            .peek(key)
            .map(|entry| entry.cached_at.elapsed() >= entry.ttl)
            .unwrap_or(false);
        if expired {
            cache.pop(key);
            return None;
        }
        cache.get(key).map(|entry| entry.data.clone())
    }

    /// Set a value in the cache with a specific TTL.
    pub fn set(&self, key: String, data: serde_json::Value, ttl: Duration) {
        let mut cache = self.cache.lock().unwrap();
        cache.put(
            key,
            CacheEntry {
                data,
                cached_at: Instant::now(),
                ttl,
            },
        );
    }

    /// Set a value in the cache with the default TTL.
    pub fn set_default(&self, key: String, data: serde_json::Value) {
        self.set(key, data, self.default_ttl);
    }

    /// Clear all entries from the cache.
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        let cache = self.cache.lock().unwrap();
        cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        let cache = self.cache.lock().unwrap();
        cache.is_empty()
    }
}

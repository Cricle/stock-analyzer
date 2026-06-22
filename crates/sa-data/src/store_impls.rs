//! `sa_models` store trait implementations for external types.
//!
//! Provides `CacheStore` for Redis (behind the `redis-cache` feature flag)
//! via a newtype wrapper around `redis::aio::ConnectionManager`.

#[cfg(feature = "redis-cache")]
use async_trait::async_trait;
#[cfg(feature = "redis-cache")]
use sa_models::store::{CacheEntry, CacheStore};

/// Redis-backed cache store.
///
/// Wraps a `redis::aio::ConnectionManager` and implements `sa_models::CacheStore`.
#[cfg(feature = "redis-cache")]
#[derive(Clone)]
pub struct RedisCacheStore {
    conn: redis::aio::ConnectionManager,
}

#[cfg(feature = "redis-cache")]
impl RedisCacheStore {
    pub fn new(conn: redis::aio::ConnectionManager) -> Self {
        Self { conn }
    }

    /// Borrow the underlying connection manager.
    pub fn conn(&self) -> &redis::aio::ConnectionManager {
        &self.conn
    }
}

#[cfg(feature = "redis-cache")]
#[async_trait]
impl CacheStore for RedisCacheStore {
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let mut conn = self.conn.clone();
        let value: Option<Vec<u8>> = redis::cmd("GET").arg(key).query_async(&mut conn).await?;
        Ok(value)
    }

    async fn set(&self, key: &str, value: &[u8], ttl_seconds: Option<u64>) -> anyhow::Result<()> {
        let mut conn = self.conn.clone();
        if let Some(ttl) = ttl_seconds {
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl)
                .arg(value)
                .exec_async(&mut conn)
                .await?;
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(value)
                .exec_async(&mut conn)
                .await?;
        }
        Ok(())
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let mut conn = self.conn.clone();
        redis::cmd("DEL").arg(key).exec_async(&mut conn).await?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        let mut conn = self.conn.clone();
        let exists: bool = redis::cmd("EXISTS").arg(key).query_async(&mut conn).await?;
        Ok(exists)
    }

    async fn list_entries(&self, _prefix: &str) -> anyhow::Result<Vec<CacheEntry>> {
        // Not implemented — would require SCAN which is expensive.
        Ok(Vec::new())
    }
}

/// No-op CacheStore implementation for fallback.
pub struct NoopCacheStore;

#[async_trait::async_trait]
impl crate::CacheStore for NoopCacheStore {
    async fn get(&self, _key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn set(&self, _key: &str, _value: &[u8], _ttl_seconds: Option<u64>) -> anyhow::Result<()> {
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

/// No-op VectorStore implementation for fallback.
pub struct NoopVectorStore;

#[async_trait::async_trait]
impl crate::VectorStore for NoopVectorStore {
    async fn insert(
        &self,
        _collection: &str,
        _id: &str,
        _embedding: &[f32],
        _payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn search(
        &self,
        _collection: &str,
        _query_embedding: &[f32],
        _top_k: usize,
    ) -> anyhow::Result<Vec<crate::VectorSearchHit>> {
        Ok(vec![])
    }
    async fn delete(&self, _collection: &str, _id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

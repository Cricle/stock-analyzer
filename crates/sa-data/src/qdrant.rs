//! Shared Qdrant HTTP client with retry logic.
//!
//! Provides collection management, upsert, search, and delete operations
//! via the Qdrant HTTP REST API, plus a `VectorStore` trait implementation.

use anyhow::Context;
use async_trait::async_trait;
use sa_models::store::{VectorSearchHit, VectorStore};
use serde_json::json;

/// Lightweight Qdrant HTTP client for vector operations.
#[derive(Clone)]
pub struct QdrantClient {
    pub http: reqwest::Client,
    pub url: String,
    pub collection: String,
}

impl QdrantClient {
    pub fn new(http: reqwest::Client, url: String, collection: String) -> Self {
        Self {
            http,
            url,
            collection,
        }
    }

    /// Ensure the collection exists with the given vector size.
    /// Creates it if missing (idempotent).
    pub async fn ensure_collection(&self, vector_size: u64) -> anyhow::Result<()> {
        let response = self
            .http
            .put(format!(
                "{}/collections/{}",
                self.url, self.collection
            ))
            .json(&json!({
                "vectors": {
                    "size": vector_size,
                    "distance": "Cosine"
                }
            }))
            .send()
            .await
            .context("failed to ensure qdrant collection")?;
        if !response.status().is_success() && response.status().as_u16() != 409 {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("qdrant ensure collection failed with {status}: {body}");
        }
        Ok(())
    }

    /// Create a payload index on the collection for a given field.
    pub async fn create_field_index(
        &self,
        field_name: &str,
        field_schema: &str,
    ) -> anyhow::Result<()> {
        let _ = self
            .http
            .put(format!(
                "{}/collections/{}/index",
                self.url, self.collection
            ))
            .json(&json!({
                "field_name": field_name,
                "field_schema": field_schema
            }))
            .send()
            .await;
        Ok(())
    }

    /// Upsert a single point into the collection.
    pub async fn upsert_point(
        &self,
        id: &str,
        vector: &[f32],
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.http
            .put(format!(
                "{}/collections/{}/points?wait=true",
                self.url, self.collection
            ))
            .json(&json!({
                "points": [{
                    "id": id,
                    "vector": vector,
                    "payload": payload
                }]
            }))
            .send()
            .await
            .context("failed to upsert qdrant point")?
            .error_for_status()
            .context("qdrant upsert request failed")?;
        Ok(())
    }

    /// Batch upsert multiple points in a single request.
    pub async fn upsert_points(
        &self,
        points: Vec<serde_json::Value>,
    ) -> anyhow::Result<()> {
        if points.is_empty() {
            return Ok(());
        }
        self.http
            .put(format!(
                "{}/collections/{}/points?wait=true",
                self.url, self.collection
            ))
            .json(&json!({ "points": points }))
            .send()
            .await
            .context("failed to batch upsert qdrant points")?
            .error_for_status()
            .context("qdrant batch upsert request failed")?;
        Ok(())
    }

    /// Search qdrant with retry. Returns the raw `result` array from the response.
    pub async fn search(
        &self,
        vector: &[f32],
        limit: usize,
        score_threshold: f64,
        must_filters: Vec<serde_json::Value>,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let url = format!(
            "{}/collections/{}/points/search",
            self.url, self.collection
        );
        let body = json!({
            "vector": vector,
            "limit": limit,
            "score_threshold": score_threshold,
            "with_payload": true,
            "filter": { "must": must_filters }
        });
        qdrant_retry("qdrant_search", 2, || {
            let url = url.clone();
            let body = body.clone();
            let http = self.http.clone();
            async move {
                let resp = http
                    .post(&url)
                    .json(&body)
                    .send()
                    .await
                    .context("failed to search qdrant")?;
                if resp.status().is_server_error() {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    anyhow::bail!("qdrant {status}: {text}");
                }
                let value: serde_json::Value = resp
                    .error_for_status()
                    .context("qdrant search request failed")?
                    .json()
                    .await
                    .context("failed to decode qdrant search response")?;
                Ok(value
                    .get("result")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default())
            }
        })
        .await
    }

    /// Delete a point by id.
    pub async fn delete_point(&self, id: &str) -> anyhow::Result<()> {
        self.http
            .post(format!(
                "{}/collections/{}/points/delete",
                self.url, self.collection
            ))
            .json(&json!({
                "points": [id]
            }))
            .send()
            .await
            .context("failed to delete qdrant point")?
            .error_for_status()
            .context("qdrant delete request failed")?;
        Ok(())
    }

    /// Delete points matching a filter.
    pub async fn delete_by_filter(
        &self,
        must_filters: Vec<serde_json::Value>,
    ) -> anyhow::Result<()> {
        self.http
            .post(format!(
                "{}/collections/{}/points/delete",
                self.url, self.collection
            ))
            .json(&json!({
                "filter": { "must": must_filters }
            }))
            .send()
            .await
            .context("failed to delete qdrant points by filter")?
            .error_for_status()
            .context("qdrant filter-delete request failed")?;
        Ok(())
    }
}

/// `VectorStore` implementation backed by Qdrant HTTP API.
///
/// The `collection` parameter in trait methods is ignored; the client uses
/// the collection configured at construction time.
#[async_trait]
impl VectorStore for QdrantClient {
    async fn insert(
        &self,
        _collection: &str,
        id: &str,
        embedding: &[f32],
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        self.upsert_point(id, embedding, payload).await
    }

    async fn search(
        &self,
        _collection: &str,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<VectorSearchHit>> {
        let raw_results = self.search(query_embedding, top_k, 0.0, vec![]).await?;
        Ok(raw_results
            .into_iter()
            .filter_map(|v| {
                let id = v.get("id")?.as_str()?.to_string();
                let score = v.get("score")?.as_f64()? as f32;
                let payload = v.get("payload").cloned().unwrap_or_default();
                Some(VectorSearchHit { id, score, payload })
            })
            .collect())
    }

    async fn delete(&self, _collection: &str, id: &str) -> anyhow::Result<()> {
        self.delete_point(id).await
    }
}

async fn qdrant_retry<F, Fut, T>(
    operation_name: &str,
    max_retries: u32,
    mut f: F,
) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut last_error = None;
    for attempt in 0..=max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                let is_retryable = error
                    .downcast_ref::<reqwest::Error>()
                    .is_some_and(|e| e.is_timeout() || e.is_connect())
                    || error.to_string().contains("5");
                if attempt < max_retries && is_retryable {
                    let delay_ms = 100 * 2u64.pow(attempt);
                    tracing::warn!(
                        op = operation_name,
                        attempt = attempt + 1,
                        max_retries = max_retries,
                        delay_ms = delay_ms,
                        error = %error,
                        "qdrant operation failed, retrying"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    last_error = Some(error);
                } else {
                    return Err(error);
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("{operation_name} retry exhausted")))
}

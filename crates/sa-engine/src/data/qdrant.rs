//! Shared Qdrant HTTP client with retry logic.

use anyhow::Context;
use serde_json::json;

/// Lightweight Qdrant HTTP client for vector search operations.
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

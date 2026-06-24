//! Vector search operations using the trait-based VectorStore.

use serde_json::json;
use super::*;

impl GuidanceStore {
    /// Search the guidance vector store.
    ///
    /// TODO: The VectorStore trait doesn't support filtered search (entry_kind, market).
    /// Results will need post-filtering on the payload. Consider extending VectorStore
    /// with a filtered search method.
    async fn vector_search(
        &self,
        embedding: &[f32],
        _entry_kind: &str,
        _market: Option<&str>,
        limit: usize,
        _score_threshold: f64,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let hits = self
            .vector_store
            .search(GUIDANCE_VECTOR_COLLECTION, embedding, limit)
            .await?;
        // Convert VectorSearchHit to the expected JSON format
        Ok(hits
            .into_iter()
            .map(|hit| {
                serde_json::json!({
                    "id": hit.id,
                    "score": hit.score,
                    "payload": hit.payload,
                })
            })
            .collect())
    }

    pub async fn search_daily_summaries(
        &self,
        embedding: &[f32],
        market: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        self.vector_search(embedding, "daily_guidance", market, limit, 0.15)
            .await
    }

    pub async fn search_news(
        &self,
        embedding: &[f32],
        market: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        self.vector_search(embedding, "news", market, limit, 0.2)
            .await
    }

    pub async fn search_sector_context(
        &self,
        query_embedding: &[f32],
        market: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let mkt = if market.trim().is_empty() {
            None
        } else {
            Some(market)
        };
        self.vector_search(query_embedding, "sector_highlight", mkt, limit, 0.15)
            .await
    }

    pub async fn search_sentiment_context(
        &self,
        query_embedding: &[f32],
        market: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let mkt = if market.trim().is_empty() {
            None
        } else {
            Some(market)
        };
        self.vector_search(query_embedding, "market_sentiment", mkt, limit, 0.15)
            .await
    }
}

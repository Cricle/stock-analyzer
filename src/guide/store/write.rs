//! Trait-based vector write operations: upsert daily summaries, news, sectors, sentiment.

use super::*;
use crate::guide::embedding::semantic_embed;

impl GuidanceStore {
    /// Ensure the vector collection exists.
    ///
    /// TODO: The VectorStore trait doesn't expose collection management.
    /// Implementations should handle collection creation internally.
    pub async fn ensure_collection(&self) -> anyhow::Result<()> {
        // No-op: VectorStore implementations are expected to manage their own collections.
        Ok(())
    }

    pub async fn store_daily_summary(
        &self,
        report: &DailyGuidanceReport,
        embedding: &[f32],
    ) -> anyhow::Result<()> {
        let point_id =
            Self::qdrant_point_id(&format!("guidance:{}:{}", report.date, report.market));
        let summary_text = format!(
            "date {} market {} sentiment {} news_count {} sector_count {} stock_count {} risk_count {}",
            report.date,
            report.market,
            report.market_sentiment.label,
            report.key_news.len(),
            report.sector_highlights.len(),
            report.stock_guidances.len(),
            report.risk_alerts.len(),
        );

        let payload = serde_json::json!({
            "entry_kind": "daily_guidance",
            "date": report.date,
            "market": report.market,
            "market_lc": report.market.to_ascii_lowercase(),
            "sentiment_score": report.market_sentiment.score,
            "sentiment_label": report.market_sentiment.label,
            "news_count": report.key_news.len(),
            "sector_count": report.sector_highlights.len(),
            "stock_count": report.stock_guidances.len(),
            "risk_count": report.risk_alerts.len(),
            "text": summary_text,
            "generated_at": report.generated_at,
        });

        self.vector_store
            .insert(GUIDANCE_VECTOR_COLLECTION, &point_id, embedding, payload)
            .await
    }

    pub async fn store_news_embedding(
        &self,
        date: &str,
        market: &str,
        title: &str,
        summary: &str,
        source: &str,
        url: Option<&str>,
        embedding: &[f32],
    ) -> anyhow::Result<()> {
        let dedup_key = Self::news_dedup_key(title, source);
        let point_id = Self::qdrant_point_id(&format!("news:{date}:{dedup_key}"));
        let text = format!("{} {} {}", title, summary, source);

        let payload = serde_json::json!({
            "entry_kind": "news",
            "date": date,
            "market": market,
            "market_lc": market.to_ascii_lowercase(),
            "title": title,
            "summary": summary,
            "source": source,
            "url": url.unwrap_or_default(),
            "dedup_key": dedup_key,
            "text": text,
        });

        self.vector_store
            .insert(GUIDANCE_VECTOR_COLLECTION, &point_id, embedding, payload)
            .await
    }

    pub async fn batch_store_news_embeddings(
        &self,
        date: &str,
        market: &str,
        news_items: &[(String, String, String, Option<String>, Vec<f32>)],
    ) -> anyhow::Result<usize> {
        if news_items.is_empty() {
            return Ok(0);
        }
        let mut count = 0usize;
        for (title, summary, source, url, embedding) in news_items {
            self.store_news_embedding(
                date,
                market,
                title,
                summary,
                source,
                url.as_deref(),
                embedding,
            )
            .await?;
            count += 1;
        }
        Ok(count)
    }

    pub async fn store_sector_embeddings(
        &self,
        report: &DailyGuidanceReport,
    ) -> anyhow::Result<usize> {
        let mut count = 0usize;
        for sector in &report.sector_highlights {
            let text = format!(
                "sector {} direction {} driver {} stocks {}",
                sector.sector_name,
                sector.direction,
                sector.key_driver,
                sector.representative_stocks.join(" ")
            );
            let embedding = semantic_embed(&text);
            let point_id = Self::qdrant_point_id(&format!(
                "sector:{}:{}:{}",
                report.date, report.market, sector.sector_name
            ));
            let payload = serde_json::json!({
                "entry_kind": "sector_highlight",
                "date": report.date,
                "market": report.market,
                "market_lc": report.market.to_ascii_lowercase(),
                "sector_name": sector.sector_name,
                "direction": sector.direction,
                "key_driver": sector.key_driver,
                "representative_stocks": sector.representative_stocks,
                "text": text,
            });
            self.vector_store
                .insert(GUIDANCE_VECTOR_COLLECTION, &point_id, &embedding, payload)
                .await?;
            count += 1;
        }

        // Store market sentiment
        let sentiment_text = format!(
            "market sentiment {} score {} drivers {}",
            report.market_sentiment.label,
            report.market_sentiment.score,
            report.market_sentiment.drivers.join(" ")
        );
        let sentiment_embedding = semantic_embed(&sentiment_text);
        let sentiment_id =
            Self::qdrant_point_id(&format!("sentiment:{}:{}", report.date, report.market));
        let payload = serde_json::json!({
            "entry_kind": "market_sentiment",
            "date": report.date,
            "market": report.market,
            "market_lc": report.market.to_ascii_lowercase(),
            "sentiment_score": report.market_sentiment.score,
            "sentiment_label": report.market_sentiment.label,
            "rationale": report.market_sentiment.rationale,
            "drivers": report.market_sentiment.drivers,
            "text": sentiment_text,
        });
        self.vector_store
            .insert(
                GUIDANCE_VECTOR_COLLECTION,
                &sentiment_id,
                &sentiment_embedding,
                payload,
            )
            .await?;
        count += 1;

        Ok(count)
    }
}

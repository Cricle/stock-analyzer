//! Historical pattern queries from Qdrant and memory system.

use super::*;
use crate::guide::embedding::semantic_embed;

impl DailyGuidanceGenerator {
    pub(super) async fn query_historical_patterns(
        &self,
        market: &GuidanceMarket,
        _date: &str,
    ) -> Vec<HistoricalInsight> {
        let mut insights = Vec::new();
        let query_text = format!(
            "market {} guidance pattern sentiment risk sector",
            market.as_str()
        );
        let embedding = semantic_embed(&query_text);

        // Search daily guidance summaries
        match self
            .store
            .search_daily_summaries(&embedding, Some(market.as_str()), QDRANT_HISTORY_LIMIT)
            .await
        {
            Ok(results) => {
                for point in &results {
                    if let Some(payload) = point.get("payload") {
                        let score = point.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let date_val = payload
                            .get("date")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let sentiment = payload
                            .get("sentiment_label")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        insights.push(HistoricalInsight {
                            pattern_type: "daily_summary".to_string(),
                            description: format!(
                                "Past guidance on {}: sentiment was {} (score: {:.2})",
                                date_val, sentiment, score
                            ),
                            relevant_tickers: Vec::new(),
                            confidence: score,
                            source: "qdrant:daily_guidance".to_string(),
                        });
                    }
                }
            }
            Err(e) => {
                tracing::warn!("qdrant daily summary search failed: {e}");
            }
        }

        // Search news history
        match self
            .store
            .search_news(&embedding, Some(market.as_str()), QDRANT_NEWS_LIMIT)
            .await
        {
            Ok(results) => {
                for point in &results {
                    if let Some(payload) = point.get("payload") {
                        let score = point.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let news_date = payload.get("date").and_then(|v| v.as_str()).unwrap_or("");
                        if score > 0.3 {
                            insights.push(HistoricalInsight {
                                pattern_type: "news_pattern".to_string(),
                                description: format!(
                                    "Similar news on {}: \"{}\"",
                                    news_date, title
                                ),
                                relevant_tickers: Vec::new(),
                                confidence: score,
                                source: "qdrant:news".to_string(),
                            });
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("qdrant news search failed: {e}");
            }
        }

        // Query the main memory system for resolved decisions
        let memory_context = self
            .memory
            .past_context_bundle(&format!("MARKET_{}", market.as_str()), 3, 3)
            .await;
        if !memory_context.context_text.is_empty() {
            insights.push(HistoricalInsight {
                pattern_type: "memory_context".to_string(),
                description: format!(
                    "Past analysis memory: {} same-ticker, {} cross-ticker matches",
                    memory_context.same_ticker_count, memory_context.cross_ticker_count
                ),
                relevant_tickers: Vec::new(),
                confidence: 0.5,
                source: memory_context.source,
            });
        }

        insights
    }
}

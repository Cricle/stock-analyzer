//! Stock-level guidance generation.

use super::*;
use crate::guide::embedding::semantic_embed;

impl DailyGuidanceGenerator {
    pub(super) async fn generate_stock_guidances(
        &self,
        tickers: &[String],
        market: &GuidanceMarket,
        news: &[GuidanceNewsItem],
    ) -> Vec<StockGuidance> {
        let mut guidances = Vec::new();

        for ticker in tickers {
            let ticker_upper = ticker.trim().to_uppercase();
            if ticker_upper.is_empty() {
                continue;
            }

            let memory_bundle = self.memory.past_context_bundle(&ticker_upper, 3, 2).await;

            let query_text = format!("{} market {} guidance", ticker_upper, market.as_str());
            let embedding = semantic_embed(&query_text);
            let stock_pick_hits = self
                .store
                .search_daily_summaries(&embedding, Some(market.as_str()), 3)
                .await
                .unwrap_or_default();

            let memory_relevance = if memory_bundle.vector_hit_count > 0 {
                0.7
            } else if !stock_pick_hits.is_empty() {
                stock_pick_hits
                    .first()
                    .and_then(|p| p.get("score").and_then(|v| v.as_f64()))
                    .unwrap_or(0.3)
            } else {
                0.0
            };

            let relevant_news: Vec<&GuidanceNewsItem> = news
                .iter()
                .filter(|n| {
                    let text = format!("{} {}", n.title, n.summary).to_ascii_lowercase();
                    text.contains(&ticker_upper.to_ascii_lowercase())
                })
                .collect();

            let guidance_action = if memory_bundle.same_ticker_count > 0 {
                "review_memory"
            } else if !relevant_news.is_empty() {
                "monitor_news"
            } else {
                "observe"
            };

            let key_risks: Vec<String> = memory_bundle
                .same_ticker_highlights
                .iter()
                .map(|h| h.key_risk.clone())
                .filter(|r| !r.trim().is_empty())
                .collect();

            guidances.push(StockGuidance {
                symbol: ticker_upper,
                stock_name: String::new(),
                market: market.as_str().to_string(),
                current_price: None,
                price_change_pct: None,
                guidance_action: guidance_action.to_string(),
                confidence: if memory_relevance > 0.5 { 70 } else { 40 },
                rationale: if memory_bundle.same_ticker_count > 0 {
                    format!(
                        "Found {} past analyses for this ticker. {}",
                        memory_bundle.same_ticker_count,
                        memory_bundle
                            .same_ticker_highlights
                            .first()
                            .map(|h| h.lesson.clone())
                            .unwrap_or_default()
                    )
                } else {
                    "Limited historical data available for this ticker.".to_string()
                },
                key_risks,
                memory_relevance,
            });
        }

        guidances
    }
}

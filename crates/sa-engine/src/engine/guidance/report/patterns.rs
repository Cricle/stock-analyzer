//! Historical pattern queries from memory system.

use super::*;

impl DailyGuidanceGenerator {
    pub(super) async fn query_historical_patterns(
        &self,
        market: &GuidanceMarket,
        _date: &str,
    ) -> Vec<HistoricalInsight> {
        let mut insights = Vec::new();

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

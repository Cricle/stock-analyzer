//! Stock-level guidance generation.

use super::*;
use crate::guide::embedding::semantic_embed;

/// Determine suggested action based on market conditions.
fn determine_suggested_action(
    sentiment: &MarketSentiment,
    relevant_news: &[&GuidanceNewsItem],
    history_count: usize,
) -> (String, String) {
    let positive_news = relevant_news
        .iter()
        .filter(|n| n.impact == "positive")
        .count();
    let negative_news = relevant_news
        .iter()
        .filter(|n| n.impact == "negative")
        .count();

    match sentiment.label.as_str() {
        "bullish" => {
            if positive_news > negative_news {
                (
                    "guidance.action.accumulate".to_string(),
                    "guidance.rationale.bullish_positive_news".to_string(),
                )
            } else if history_count > 0 {
                (
                    "guidance.action.review_memory".to_string(),
                    "guidance.rationale.bullish_mixed_news".to_string(),
                )
            } else {
                (
                    "guidance.action.watch_for_pullback".to_string(),
                    "guidance.rationale.bullish_mixed_news".to_string(),
                )
            }
        }
        "bearish" => {
            if negative_news > 0 {
                (
                    "guidance.action.avoid".to_string(),
                    "guidance.rationale.bearish_negative_news".to_string(),
                )
            } else {
                (
                    "guidance.action.wait_for_confirmation".to_string(),
                    "guidance.rationale.bearish_negative_news".to_string(),
                )
            }
        }
        _ => {
            if positive_news > negative_news + 1 {
                (
                    "guidance.action.watch_for_pullback".to_string(),
                    "guidance.rationale.neutral_positive_bias".to_string(),
                )
            } else if negative_news > positive_news + 1 {
                (
                    "guidance.action.monitor".to_string(),
                    "guidance.rationale.neutral_negative_bias".to_string(),
                )
            } else {
                (
                    "guidance.action.observe".to_string(),
                    "guidance.rationale.neutral_mixed".to_string(),
                )
            }
        }
    }
}

/// Adjust confidence based on market sentiment.
fn adjust_confidence_for_sentiment(base: i32, sentiment: &MarketSentiment) -> i32 {
    let adjustment = match sentiment.label.as_str() {
        "bullish" => 10,
        "slightly_bullish" => 5,
        "bearish" => -15,
        "slightly_bearish" => -5,
        _ => 0,
    };
    (base + adjustment).clamp(20, 90)
}

impl DailyGuidanceGenerator {
    pub(super) async fn generate_stock_guidances(
        &self,
        tickers: &[String],
        market: &GuidanceMarket,
        news: &[GuidanceNewsItem],
        sentiment: &MarketSentiment,
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

            // Determine suggested action based on sentiment and news
            let (suggested_action, action_rationale) = determine_suggested_action(
                sentiment,
                &relevant_news,
                memory_bundle.same_ticker_count,
            );

            // Adjust confidence based on sentiment
            let base_confidence = if memory_relevance > 0.5 { 70 } else { 40 };
            let confidence = adjust_confidence_for_sentiment(base_confidence, sentiment);

            let key_risks: Vec<String> = memory_bundle
                .same_ticker_highlights
                .iter()
                .map(|h| h.key_risk.clone())
                .filter(|r| !r.trim().is_empty())
                .collect();

            // Build rationale with actionable context
            let rationale = if memory_bundle.same_ticker_count > 0 {
                format!(
                    "guidance.rationale.has_history|count={}|lesson={}",
                    memory_bundle.same_ticker_count,
                    memory_bundle
                        .same_ticker_highlights
                        .first()
                        .map(|h| h.lesson.clone())
                        .unwrap_or_default()
                )
            } else {
                format!(
                    "guidance.rationale.limited_history|news_count={}",
                    relevant_news.len()
                )
            };

            guidances.push(StockGuidance {
                symbol: ticker_upper,
                stock_name: String::new(),
                market: market.as_str().to_string(),
                current_price: None,
                price_change_pct: None,
                guidance_action: guidance_action.to_string(),
                confidence,
                rationale,
                key_risks,
                memory_relevance,
                entry_zone: None,
                resistance_level: None,
                suggested_action,
                action_rationale,
                key_levels: vec![],
            });
        }

        guidances
    }
}

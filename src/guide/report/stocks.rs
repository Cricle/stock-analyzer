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
                    "accumulate".to_string(),
                    "Bullish market with positive news flow. Consider accumulating on dips."
                        .to_string(),
                )
            } else if history_count > 0 {
                (
                    "review_memory".to_string(),
                    "Bullish market but mixed news. Review past analysis for entry timing."
                        .to_string(),
                )
            } else {
                (
                    "watch_for_pullback".to_string(),
                    "Bullish market but limited data. Wait for pullback entry.".to_string(),
                )
            }
        }
        "bearish" => {
            if negative_news > 0 {
                (
                    "avoid".to_string(),
                    "Bearish market with negative news. Avoid new entries, consider reducing exposure.".to_string(),
                )
            } else {
                (
                    "wait_for_confirmation".to_string(),
                    "Bearish market. Wait for reversal confirmation before entry.".to_string(),
                )
            }
        }
        _ => {
            if positive_news > negative_news + 1 {
                (
                    "watch_for_pullback".to_string(),
                    "Neutral market with positive news bias. Watch for pullback entry.".to_string(),
                )
            } else if negative_news > positive_news + 1 {
                (
                    "monitor".to_string(),
                    "Neutral market with negative news bias. Monitor for deterioration."
                        .to_string(),
                )
            } else {
                (
                    "observe".to_string(),
                    "Neutral market conditions. Observe and wait for clearer signals.".to_string(),
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
                    "Found {} past analyses for this ticker. {} Suggested action: {}.",
                    memory_bundle.same_ticker_count,
                    memory_bundle
                        .same_ticker_highlights
                        .first()
                        .map(|h| h.lesson.clone())
                        .unwrap_or_default(),
                    suggested_action
                )
            } else {
                format!(
                    "Limited historical data available for this ticker. {}. Suggested action: {}.",
                    if !relevant_news.is_empty() {
                        format!("{} relevant news items found", relevant_news.len())
                    } else {
                        "No significant news".to_string()
                    },
                    suggested_action
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

//! Stock-level guidance generation.

use super::*;
use crate::guide::embedding::semantic_embed;
use crate::guide::models::I18nText;

/// Determine suggested action based on market conditions.
fn determine_suggested_action(
    sentiment: &MarketSentiment,
    relevant_news: &[&GuidanceNewsItem],
    history_count: usize,
) -> (I18nText, I18nText) {
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
                    I18nText::new("guidance.action.accumulate"),
                    I18nText::new("guidance.rationale.bullish_positive_news"),
                )
            } else if history_count > 0 {
                (
                    I18nText::new("guidance.action.review_memory"),
                    I18nText::new("guidance.rationale.bullish_mixed_news"),
                )
            } else {
                (
                    I18nText::new("guidance.action.watch_for_pullback"),
                    I18nText::new("guidance.rationale.bullish_limited_data"),
                )
            }
        }
        "bearish" => {
            if negative_news > 0 {
                (
                    I18nText::new("guidance.action.avoid"),
                    I18nText::new("guidance.rationale.bearish_negative_news"),
                )
            } else {
                (
                    I18nText::new("guidance.action.wait_for_confirmation"),
                    I18nText::new("guidance.rationale.bearish_no_news"),
                )
            }
        }
        _ => {
            if positive_news > negative_news + 1 {
                (
                    I18nText::new("guidance.action.watch_for_pullback"),
                    I18nText::new("guidance.rationale.neutral_positive_bias"),
                )
            } else if negative_news > positive_news + 1 {
                (
                    I18nText::new("guidance.action.monitor"),
                    I18nText::new("guidance.rationale.neutral_negative_bias"),
                )
            } else {
                (
                    I18nText::new("guidance.action.observe"),
                    I18nText::new("guidance.rationale.neutral_mixed"),
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
                I18nText::new("guidance.action.review_memory")
            } else if !relevant_news.is_empty() {
                I18nText::new("guidance.action.monitor_news")
            } else {
                I18nText::new("guidance.action.observe")
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

            let key_risks: Vec<I18nText> = memory_bundle
                .same_ticker_highlights
                .iter()
                .map(|h| I18nText::new(&h.key_risk))
                .filter(|r| !r.key.trim().is_empty())
                .collect();

            // Build rationale with actionable context
            let rationale = if memory_bundle.same_ticker_count > 0 {
                I18nText::new("guidance.rationale.has_history")
                    .with_param("count", memory_bundle.same_ticker_count as i64)
                    .with_param(
                        "lesson",
                        memory_bundle
                            .same_ticker_highlights
                            .first()
                            .map(|h| h.lesson.clone())
                            .unwrap_or_default(),
                    )
            } else {
                I18nText::new("guidance.rationale.limited_history")
                    .with_param("news_count", relevant_news.len() as i64)
            };

            guidances.push(StockGuidance {
                symbol: ticker_upper,
                stock_name: String::new(),
                market: market.as_str().to_string(),
                current_price: None,
                price_change_pct: None,
                guidance_action,
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

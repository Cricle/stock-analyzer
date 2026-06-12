//! Risk alert generation from news and sentiment.

use super::*;

impl DailyGuidanceGenerator {
    pub(super) fn generate_risk_alerts(
        &self,
        news: &[GuidanceNewsItem],
        sentiment: &MarketSentiment,
        market: &GuidanceMarket,
        indices: &[MarketIndex],
    ) -> Vec<RiskAlert> {
        let mut alerts = Vec::new();

        // Sentiment-based alert (lowered threshold: -10 instead of -20)
        if sentiment.score < -10 {
            alerts.push(RiskAlert {
                severity: if sentiment.score < -20 { "high" } else { "medium" }.to_string(),
                category: "market_sentiment".to_string(),
                description: format!(
                    "Market sentiment is bearish (score: {}). Consider reducing exposure.",
                    sentiment.score
                ),
                mitigation: "Review stop-loss levels and position sizing.".to_string(),
                affected_markets: vec![market.as_str().to_string()],
            });
        }

        // Negative news count alert (lowered threshold: >= 2 instead of > 3)
        let neg_count = news.iter().filter(|n| n.impact == "negative").count();
        if neg_count >= 2 {
            let neg_titles: Vec<&str> = news
                .iter()
                .filter(|n| n.impact == "negative")
                .take(3)
                .map(|n| n.title.as_str())
                .collect();
            alerts.push(RiskAlert {
                severity: if neg_count >= 4 { "high" } else { "medium" }.to_string(),
                category: "news_flow".to_string(),
                description: format!(
                    "Multiple negative news items ({}): {}",
                    neg_count,
                    truncate_event_titles(&neg_titles)
                ),
                mitigation: "Diversify holdings and avoid concentrated positions.".to_string(),
                affected_markets: vec![market.as_str().to_string()],
            });
        }

        // High negative sentiment
        if !news.is_empty() {
            let avg_impact: f64 = news
                .iter()
                .map(|n| match n.impact.as_str() {
                    "positive" => 1.0,
                    "negative" => -1.0,
                    _ => 0.0,
                })
                .sum::<f64>()
                / news.len() as f64;
            if avg_impact < 0.0 && neg_count >= 2 {
                alerts.push(RiskAlert {
                    severity: "high".to_string(),
                    category: "high_negative_sentiment".to_string(),
                    description: format!(
                        "Average news impact is negative ({:.2}) with {} negative items.",
                        avg_impact, neg_count
                    ),
                    mitigation: "Consider reducing position sizes and tightening stop-losses."
                        .to_string(),
                    affected_markets: vec![market.as_str().to_string()],
                });
            }
        }

        // Sector divergence (lowered threshold: 1.5% instead of 2%)
        if indices.len() >= 2 {
            let has_up = indices.iter().any(|i| i.change_pct > 1.5);
            let has_down = indices.iter().any(|i| i.change_pct < -1.5);
            if has_up && has_down {
                let up_names: Vec<&str> = indices
                    .iter()
                    .filter(|i| i.change_pct > 1.5)
                    .map(|i| i.name.as_str())
                    .collect();
                let down_names: Vec<&str> = indices
                    .iter()
                    .filter(|i| i.change_pct < -1.5)
                    .map(|i| i.name.as_str())
                    .collect();
                alerts.push(RiskAlert {
                    severity: "medium".to_string(),
                    category: "sector_divergence".to_string(),
                    description: format!(
                        "Market indices diverging: {} rising while {} falling. \
                         Indicates sector rotation or market fragmentation.",
                        up_names.join(", "),
                        down_names.join(", ")
                    ),
                    mitigation: "Review portfolio concentration and consider hedging.".to_string(),
                    affected_markets: vec![market.as_str().to_string()],
                });
            }
        }

        alerts
    }
}

/// Truncate event titles for risk alert descriptions.
fn truncate_event_titles(titles: &[&str]) -> String {
    let mut result = String::new();
    for (i, title) in titles.iter().enumerate() {
        if i > 0 {
            result.push_str("; ");
        }
        let truncated = if title.len() > 30 {
            format!("{}...", &title[..title.floor_char_boundary(30)])
        } else {
            title.to_string()
        };
        if result.len() + truncated.len() > 100 {
            break;
        }
        result.push_str(&truncated);
    }
    result
}

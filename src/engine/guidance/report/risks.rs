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

        if sentiment.score < -20 {
            alerts.push(RiskAlert {
                severity: "high".to_string(),
                category: "market_sentiment".to_string(),
                description: format!(
                    "Market sentiment is bearish (score: {}). Consider reducing exposure.",
                    sentiment.score
                ),
                mitigation: "Review stop-loss levels and position sizing.".to_string(),
                affected_markets: vec![market.as_str().to_string()],
            });
        }

        let neg_count = news.iter().filter(|n| n.impact == "negative").count();
        if neg_count > 3 {
            alerts.push(RiskAlert {
                severity: "medium".to_string(),
                category: "news_flow".to_string(),
                description: format!(
                    "High volume of negative news ({} items). Monitor for systemic risks.",
                    neg_count
                ),
                mitigation: "Diversify holdings and avoid concentrated positions.".to_string(),
                affected_markets: vec![market.as_str().to_string()],
            });
        }

        // High negative sentiment: average impact is negative and there are
        // more than 2 negative news items.
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
            if avg_impact < 0.0 && neg_count > 2 {
                alerts.push(RiskAlert {
                    severity: "high".to_string(),
                    category: "high_negative_sentiment".to_string(),
                    description: format!(
                        "Average news impact is negative ({:.2}) with {} negative items. \
                         Sentiment deterioration warrants caution.",
                        avg_impact, neg_count
                    ),
                    mitigation: "Consider reducing position sizes and tightening stop-losses."
                        .to_string(),
                    affected_markets: vec![market.as_str().to_string()],
                });
            }
        }

        // Sector divergence: market indices show mixed signals
        if indices.len() >= 2 {
            let has_up = indices.iter().any(|i| i.change_pct > 2.0);
            let has_down = indices.iter().any(|i| i.change_pct < -2.0);
            if has_up && has_down {
                let up_names: Vec<&str> = indices
                    .iter()
                    .filter(|i| i.change_pct > 2.0)
                    .map(|i| i.name.as_str())
                    .collect();
                let down_names: Vec<&str> = indices
                    .iter()
                    .filter(|i| i.change_pct < -2.0)
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

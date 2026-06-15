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
        let market_str = market.as_str().to_string();

        // Sentiment-based alert
        if sentiment.score < -10 {
            alerts.push(RiskAlert {
                severity: if sentiment.score < -20 { "high" } else { "medium" }.to_string(),
                category: "market_sentiment".to_string(),
                description: format!(
                    "Market sentiment is bearish (score: {}). Consider reducing exposure.",
                    sentiment.score
                ),
                description_key: Some(serde_json::json!({
                    "i18n_key": "guidance.risk.sentiment_bearish",
                    "score": sentiment.score,
                })),
                mitigation: "Review stop-loss levels and position sizing.".to_string(),
                mitigation_key: Some("guidance.risk.mitigation.review_stoploss".to_string()),
                affected_markets: vec![market_str.clone()],
            });
        }

        // Negative news count alert
        let neg_count = news.iter().filter(|n| n.impact == "negative").count();
        if neg_count >= 2 {
            let neg_titles: Vec<&str> = news
                .iter()
                .filter(|n| n.impact == "negative")
                .take(3)
                .map(|n| n.title.as_str())
                .collect();
            let titles_str = truncate_event_titles(&neg_titles);
            alerts.push(RiskAlert {
                severity: if neg_count >= 4 { "high" } else { "medium" }.to_string(),
                category: "news_flow".to_string(),
                description: format!("Multiple negative news items ({}): {}", neg_count, titles_str),
                description_key: Some(serde_json::json!({
                    "i18n_key": "guidance.risk.negative_news",
                    "count": neg_count,
                    "titles": titles_str,
                })),
                mitigation: "Diversify holdings and avoid concentrated positions.".to_string(),
                mitigation_key: Some("guidance.risk.mitigation.diversify".to_string()),
                affected_markets: vec![market_str.clone()],
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
                    description_key: Some(serde_json::json!({
                        "i18n_key": "guidance.risk.high_negative_sentiment",
                        "avg": format!("{:.2}", avg_impact),
                        "count": neg_count,
                    })),
                    mitigation: "Consider reducing position sizes and tightening stop-losses."
                        .to_string(),
                    mitigation_key: Some("guidance.risk.mitigation.reduce_position".to_string()),
                    affected_markets: vec![market_str.clone()],
                });
            }
        }

        // Sector divergence
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
                    description_key: Some(serde_json::json!({
                        "i18n_key": "guidance.risk.sector_divergence",
                        "up_names": up_names.join(", "),
                        "down_names": down_names.join(", "),
                    })),
                    mitigation: "Review portfolio concentration and consider hedging.".to_string(),
                    mitigation_key: Some("guidance.risk.mitigation.hedge".to_string()),
                    affected_markets: vec![market_str.clone()],
                });
            }
        }

        // Index-level risks from market indices data
        for idx in indices {
            if idx.change_pct.abs() > 2.0 {
                alerts.push(RiskAlert {
                    severity: if idx.change_pct.abs() > 4.0 {
                        "high"
                    } else {
                        "medium"
                    }
                    .to_string(),
                    category: "index_volatility".to_string(),
                    description: format!(
                        "{} moved {:.2}% today. Significant intraday movement increases risk.",
                        idx.name, idx.change_pct
                    ),
                    description_key: Some(serde_json::json!({
                        "i18n_key": "guidance.risk.index_volatility",
                        "index": idx.name,
                        "change_pct": idx.change_pct,
                    })),
                    mitigation: "Avoid chasing momentum; wait for volatility to subside.".to_string(),
                    mitigation_key: Some("guidance.risk.mitigation.wait_volatility".to_string()),
                    affected_markets: vec![market_str.clone()],
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

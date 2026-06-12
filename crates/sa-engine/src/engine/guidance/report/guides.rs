//! User profile guide generation.

use super::*;

impl DailyGuidanceGenerator {
    pub(super) fn generate_user_guides(
        &self,
        sentiment: &MarketSentiment,
        stock_guidances: &[StockGuidance],
        risk_alerts: &[RiskAlert],
        indices: &[MarketIndex],
        sector_highlights: &[SectorHighlight],
    ) -> Vec<UserProfileGuide> {
        let has_high_risk = risk_alerts.iter().any(|r| r.severity == "high");
        let has_medium_risk = risk_alerts.iter().any(|r| r.severity == "medium");

        // Build index context string
        let index_context = if indices.is_empty() {
            String::new()
        } else {
            let parts: Vec<String> = indices
                .iter()
                .map(|i| format!("{} {:.2}%", i.name, i.change_pct))
                .collect();
            format!(" ({})", parts.join(", "))
        };

        // Extract sector info with key drivers
        let bullish_sectors: Vec<(&str, &str)> = sector_highlights
            .iter()
            .filter(|s| s.direction == "positive")
            .map(|s| (s.sector_name.as_str(), s.key_driver.as_str()))
            .collect();
        let bearish_sectors: Vec<(&str, &str)> = sector_highlights
            .iter()
            .filter(|s| s.direction == "negative")
            .map(|s| (s.sector_name.as_str(), s.key_driver.as_str()))
            .collect();

        // Build sector event summaries
        let bullish_summary = build_sector_summary(&bullish_sectors);
        let bearish_summary = build_sector_summary(&bearish_sectors);

        vec![
            UserProfileGuide {
                profile: "conservative".to_string(),
                summary: if has_high_risk {
                    format!(
                        "High risk environment{}. Avoid: {}.",
                        index_context,
                        if bearish_summary.is_empty() {
                            "high-volatility sectors".to_string()
                        } else {
                            bearish_summary.clone()
                        }
                    )
                } else if has_medium_risk {
                    format!(
                        "Elevated risk{}. Watch: {}.",
                        index_context,
                        if bullish_summary.is_empty() {
                            "quality large-caps".to_string()
                        } else {
                            bullish_summary.clone()
                        }
                    )
                } else {
                    format!(
                        "Market conditions stable{}. Hold existing positions.",
                        index_context
                    )
                },
                recommended_actions: if has_high_risk {
                    vec![
                        "Reduce equity exposure".to_string(),
                        "Increase cash allocation".to_string(),
                        "Tighten stop-losses on all positions".to_string(),
                    ]
                } else if has_medium_risk {
                    vec![
                        "Hold existing positions".to_string(),
                        "Review stop-losses".to_string(),
                        "Avoid new speculative entries".to_string(),
                    ]
                } else {
                    vec![
                        "Hold existing positions".to_string(),
                        "Review portfolio allocation".to_string(),
                    ]
                },
                watch_list: stock_guidances
                    .iter()
                    .filter(|g| g.confidence > 60)
                    .map(|g| g.symbol.clone())
                    .collect(),
                avoid_list: stock_guidances
                    .iter()
                    .filter(|g| g.key_risks.len() > 2)
                    .map(|g| g.symbol.clone())
                    .chain(bearish_sectors.iter().map(|(s, _)| format!("{} sector", s)))
                    .collect(),
            },
            UserProfileGuide {
                profile: "balanced".to_string(),
                summary: format!(
                    "Sentiment: {}{}. {}",
                    sentiment.label,
                    index_context,
                    if !bullish_summary.is_empty() && !bearish_summary.is_empty() {
                        format!("Strong: {}. Weak: {}.", bullish_summary, bearish_summary)
                    } else if !bullish_summary.is_empty() {
                        format!("Strong sectors: {}.", bullish_summary)
                    } else if !bearish_summary.is_empty() {
                        format!("Weak sectors: {}.", bearish_summary)
                    } else {
                        "No dominant sector trend.".to_string()
                    }
                ),
                recommended_actions: if sentiment.score > 10 {
                    vec![
                        "Consider increasing equity allocation".to_string(),
                        "Rotate into outperforming sectors".to_string(),
                        "Review portfolio balance".to_string(),
                    ]
                } else if sentiment.score < -10 {
                    vec![
                        "Reduce equity allocation slightly".to_string(),
                        "Increase defensive positions".to_string(),
                        "Review portfolio balance".to_string(),
                    ]
                } else {
                    vec![
                        "Review portfolio balance".to_string(),
                        "Consider sector rotation".to_string(),
                        "Maintain current allocation".to_string(),
                    ]
                },
                watch_list: stock_guidances
                    .iter()
                    .filter(|g| g.memory_relevance > 0.4)
                    .map(|g| g.symbol.clone())
                    .collect(),
                avoid_list: bearish_sectors
                    .iter()
                    .map(|(s, _)| format!("{} sector", s))
                    .collect(),
            },
            UserProfileGuide {
                profile: "aggressive".to_string(),
                summary: if sentiment.score > 20 {
                    format!(
                        "Favorable conditions{}. Focus on: {}.",
                        index_context,
                        if bullish_summary.is_empty() {
                            "breakout candidates".to_string()
                        } else {
                            bullish_summary.clone()
                        }
                    )
                } else if sentiment.score < -20 {
                    format!(
                        "Bearish environment{}. Consider hedging in: {}.",
                        index_context,
                        if bearish_summary.is_empty() {
                            "weak sectors".to_string()
                        } else {
                            bearish_summary.clone()
                        }
                    )
                } else {
                    format!(
                        "Mixed conditions{}. {}",
                        index_context,
                        if !bullish_summary.is_empty() {
                            format!("Best opportunities in {}.", bullish_summary)
                        } else {
                            "Be selective with entries.".to_string()
                        }
                    )
                },
                recommended_actions: if sentiment.score > 20 {
                    vec![
                        "Look for momentum breakouts".to_string(),
                        "Increase position size on winners".to_string(),
                        "Monitor volume breakouts".to_string(),
                    ]
                } else if sentiment.score < -20 {
                    vec![
                        "Consider hedging strategies".to_string(),
                        "Look for oversold bounces in quality names".to_string(),
                        "Tighten risk management".to_string(),
                    ]
                } else {
                    vec![
                        "Look for oversold bounces".to_string(),
                        "Monitor volume breakouts".to_string(),
                        "Wait for confirmation before entry".to_string(),
                    ]
                },
                watch_list: stock_guidances.iter().map(|g| g.symbol.clone()).collect(),
                avoid_list: bearish_sectors
                    .iter()
                    .map(|(s, _)| format!("{} sector", s))
                    .collect(),
            },
        ]
    }
}

/// Build a concise summary of sector events for user guides.
fn build_sector_summary(sectors: &[(&str, &str)]) -> String {
    let mut parts = Vec::new();
    for (name, driver) in sectors.iter().take(3) {
        let short_driver = if driver.len() > 30 {
            format!("{}...", &driver[..driver.floor_char_boundary(30)])
        } else {
            driver.to_string()
        };
        if short_driver.is_empty() {
            parts.push(name.to_string());
        } else {
            parts.push(format!("{}({})", name, short_driver));
        }
    }
    parts.join(", ")
}

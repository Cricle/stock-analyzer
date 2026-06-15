//! User profile guide generation.

use super::*;

/// Map an action string to its i18n key.
fn action_i18n_key(action: &str) -> &'static str {
    match action {
        "Hold existing positions" => "guidance.guide.action.hold",
        "Review portfolio allocation" => "guidance.guide.action.review_allocation",
        "Reduce equity exposure" => "guidance.guide.action.reduce_equity",
        "Increase cash allocation" => "guidance.guide.action.increase_cash",
        "Tighten stop-losses on all positions" => "guidance.guide.action.tighten_stoploss",
        "Review stop-losses" => "guidance.guide.action.review_stoploss",
        "Avoid new speculative entries" => "guidance.guide.action.avoid_speculative",
        "Consider increasing equity allocation" => "guidance.guide.action.increase_equity",
        "Rotate into outperforming sectors" => "guidance.guide.action.rotate_sectors",
        "Review portfolio balance" => "guidance.guide.action.review_balance",
        "Reduce equity allocation slightly" => "guidance.guide.action.reduce_equity_slightly",
        "Increase defensive positions" => "guidance.guide.action.increase_defensive",
        "Look for momentum breakouts" => "guidance.guide.action.momentum_breakout",
        "Increase position size on winners" => "guidance.guide.action.increase_winners",
        "Monitor volume breakouts" => "guidance.guide.action.monitor_volume",
        "Consider hedging strategies" => "guidance.guide.action.hedging_strategies",
        "Look for oversold bounces in quality names" => "guidance.guide.action.oversold_bounces",
        "Tighten risk management" => "guidance.guide.action.risk_management",
        "Wait for confirmation before entry" => "guidance.guide.action.wait_confirmation",
        "Maintain current allocation" => "guidance.guide.action.maintain_allocation",
        "Consider sector rotation" => "guidance.guide.action.consider_sector_rotation",
        _ => "guidance.guide.action.hold",
    }
}

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

        // Conservative profile
        let (cons_summary, cons_summary_key) = if has_high_risk {
            let avoid = if bearish_summary.is_empty() {
                "high-volatility sectors".to_string()
            } else {
                bearish_summary.clone()
            };
            (
                format!("High risk environment{}. Avoid: {}.", index_context, avoid),
                serde_json::json!({
                    "i18n_key": "guidance.guide.conservative.high_risk",
                    "index_context": index_context,
                    "avoid": avoid,
                }),
            )
        } else if has_medium_risk {
            let watch = if bullish_summary.is_empty() {
                "quality large-caps".to_string()
            } else {
                bullish_summary.clone()
            };
            (
                format!("Elevated risk{}. Watch: {}.", index_context, watch),
                serde_json::json!({
                    "i18n_key": "guidance.guide.conservative.medium_risk",
                    "index_context": index_context,
                    "watch": watch,
                }),
            )
        } else {
            (
                format!("Market conditions stable{}. Hold existing positions.", index_context),
                serde_json::json!({
                    "i18n_key": "guidance.guide.conservative.stable",
                    "index_context": index_context,
                }),
            )
        };
        let cons_actions: Vec<String> = if has_high_risk {
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
        };

        // Balanced profile
        let (sector_info, sector_info_key) = if !bullish_summary.is_empty() && !bearish_summary.is_empty() {
            (
                format!("Strong: {}. Weak: {}.", bullish_summary, bearish_summary),
                serde_json::json!({
                    "i18n_key": "guidance.guide.sector_info_both",
                    "strong": bullish_summary,
                    "weak": bearish_summary,
                }),
            )
        } else if !bullish_summary.is_empty() {
            (
                format!("Strong sectors: {}.", bullish_summary),
                serde_json::json!({
                    "i18n_key": "guidance.guide.sector_info_strong",
                    "info": bullish_summary,
                }),
            )
        } else if !bearish_summary.is_empty() {
            (
                format!("Weak sectors: {}.", bearish_summary),
                serde_json::json!({
                    "i18n_key": "guidance.guide.sector_info_weak",
                    "info": bearish_summary,
                }),
            )
        } else {
            (
                "No dominant sector trend.".to_string(),
                serde_json::json!({"i18n_key": "guidance.guide.no_sector_trend"}),
            )
        };
        let (bal_summary, bal_summary_key) = (
            format!("Sentiment: {}{}. {}", sentiment.label, index_context, sector_info),
            serde_json::json!({
                "i18n_key": "guidance.guide.balanced.summary",
                "label": sentiment.label,
                "index_context": index_context,
                "sector_info": sector_info,
            }),
        );
        let bal_actions: Vec<String> = if sentiment.score > 10 {
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
        };

        // Aggressive profile
        let (agg_summary, agg_summary_key) = if sentiment.score > 20 {
            let focus = if bullish_summary.is_empty() {
                "breakout candidates".to_string()
            } else {
                bullish_summary.clone()
            };
            (
                format!("Favorable conditions{}. Focus on: {}.", index_context, focus),
                serde_json::json!({
                    "i18n_key": "guidance.guide.aggressive.bullish",
                    "index_context": index_context,
                    "focus": focus,
                }),
            )
        } else if sentiment.score < -20 {
            let hedge = if bearish_summary.is_empty() {
                "weak sectors".to_string()
            } else {
                bearish_summary.clone()
            };
            (
                format!("Bearish environment{}. Consider hedging in: {}.", index_context, hedge),
                serde_json::json!({
                    "i18n_key": "guidance.guide.aggressive.bearish",
                    "index_context": index_context,
                    "hedge": hedge,
                }),
            )
        } else {
            let info = if !bullish_summary.is_empty() {
                format!("Best opportunities in {}.", bullish_summary)
            } else {
                "Be selective with entries.".to_string()
            };
            (
                format!("Mixed conditions{}. {}", index_context, info),
                serde_json::json!({
                    "i18n_key": "guidance.guide.aggressive.mixed",
                    "index_context": index_context,
                    "info": info,
                }),
            )
        };
        let agg_actions: Vec<String> = if sentiment.score > 20 {
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
        };

        vec![
            UserProfileGuide {
                profile: "conservative".to_string(),
                profile_key: Some("guidance.profile.conservative".to_string()),
                summary: cons_summary,
                summary_key: Some(cons_summary_key),
                recommended_actions: cons_actions.iter().map(|a| action_i18n_key(a).to_string()).collect(),
                recommended_action_keys: Some(cons_actions.iter().map(|a| action_i18n_key(a).to_string()).collect()),
                action_texts: cons_actions,
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
                sector_info_key: None,
            },
            UserProfileGuide {
                profile: "balanced".to_string(),
                profile_key: Some("guidance.profile.balanced".to_string()),
                summary: bal_summary,
                summary_key: Some(bal_summary_key),
                recommended_actions: bal_actions.iter().map(|a| action_i18n_key(a).to_string()).collect(),
                recommended_action_keys: Some(bal_actions.iter().map(|a| action_i18n_key(a).to_string()).collect()),
                action_texts: bal_actions,
                watch_list: stock_guidances
                    .iter()
                    .filter(|g| g.memory_relevance > 0.4)
                    .map(|g| g.symbol.clone())
                    .collect(),
                avoid_list: bearish_sectors
                    .iter()
                    .map(|(s, _)| format!("{} sector", s))
                    .collect(),
                sector_info_key: Some(sector_info_key),
            },
            UserProfileGuide {
                profile: "aggressive".to_string(),
                profile_key: Some("guidance.profile.aggressive".to_string()),
                summary: agg_summary,
                summary_key: Some(agg_summary_key),
                recommended_actions: agg_actions.iter().map(|a| action_i18n_key(a).to_string()).collect(),
                recommended_action_keys: Some(agg_actions.iter().map(|a| action_i18n_key(a).to_string()).collect()),
                action_texts: agg_actions,
                watch_list: stock_guidances.iter().map(|g| g.symbol.clone()).collect(),
                avoid_list: bearish_sectors
                    .iter()
                    .map(|(s, _)| format!("{} sector", s))
                    .collect(),
                sector_info_key: None,
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

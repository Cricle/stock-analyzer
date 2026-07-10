//! Stock pick validation and quality gates.

use crate::pick::types::GeneratedStockPickItem;
use crate::pick::EnrichedCandidate;

/// Configuration for pick quality gates.
#[derive(Debug, Clone)]
pub struct PickQualityGate {
    pub min_risk_reward_ratio: f64,
    pub require_catalyst: bool,
    pub require_exit_strategy: bool,
    pub max_stop_loss_pct: f64,
}

impl Default for PickQualityGate {
    fn default() -> Self {
        Self {
            min_risk_reward_ratio: 1.5,
            require_catalyst: true,
            require_exit_strategy: true,
            max_stop_loss_pct: 10.0,
        }
    }
}

/// Result of validating a stock pick.
#[derive(Debug, Clone)]
pub struct PickValidation {
    pub has_entry_price: bool,
    pub has_stop_loss: bool,
    pub has_target: bool,
    pub risk_reward_ratio: f64,
    pub has_catalyst: bool,
    pub has_exit_strategy: bool,
    pub is_valid: bool,
    pub issues: Vec<String>,
}

/// Parse a price string like "150" or "150-155" to a numeric value (takes first number).
fn parse_price(s: &str) -> Option<f64> {
    s.trim()
        .split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse::<f64>().ok())
}

/// Validate a stock pick against quality gates.
pub(crate) fn validate_pick(
    pick: &GeneratedStockPickItem,
    _current_price: Option<f64>,
    config: &PickQualityGate,
) -> PickValidation {
    let mut issues = Vec::new();

    let has_entry_price = pick.entry_price.as_ref().is_some_and(|s| !s.trim().is_empty());
    let has_stop_loss = pick.stop_loss.as_ref().is_some_and(|s| !s.trim().is_empty());
    let has_target = pick.target_price.as_ref().is_some_and(|s| !s.trim().is_empty());
    let has_catalyst = !pick.catalysts.is_empty();
    let has_exit_strategy = !pick.exit_triggers.is_empty();

    if !has_entry_price {
        issues.push("missing entry_price".to_string());
    }
    if !has_stop_loss {
        issues.push("missing stop_loss".to_string());
    }
    if !has_target {
        issues.push("missing target_price".to_string());
    }
    if config.require_catalyst && !has_catalyst {
        issues.push("missing catalysts".to_string());
    }
    if config.require_exit_strategy && !has_exit_strategy {
        issues.push("missing exit_triggers".to_string());
    }

    // Calculate R/R ratio
    let risk_reward_ratio = match (
        pick.entry_price.as_ref().and_then(|s| parse_price(s)),
        pick.stop_loss.as_ref().and_then(|s| parse_price(s)),
        pick.target_price.as_ref().and_then(|s| parse_price(s)),
    ) {
        (Some(entry), Some(stop), Some(target)) if entry > 0.0 => {
            let risk = (entry - stop).abs();
            let reward = (target - entry).abs();
            if risk > 0.0 {
                reward / risk
            } else {
                0.0
            }
        }
        _ => 0.0,
    };

    if risk_reward_ratio > 0.0 && risk_reward_ratio < config.min_risk_reward_ratio {
        issues.push(format!(
            "risk/reward ratio {:.2} below minimum {:.2}",
            risk_reward_ratio, config.min_risk_reward_ratio
        ));
    }

    // Check stop loss is below entry for long positions
    if let (Some(entry_str), Some(stop_str)) = (&pick.entry_price, &pick.stop_loss) {
        if let (Some(entry), Some(stop)) = (parse_price(entry_str), parse_price(stop_str)) {
            if stop >= entry {
                issues.push("stop_loss must be below entry_price for long positions".to_string());
            }
        }
    }

    // Check stop loss percentage
    if let (Some(entry_str), Some(stop_str)) = (&pick.entry_price, &pick.stop_loss) {
        if let (Some(entry), Some(stop)) = (parse_price(entry_str), parse_price(stop_str)) {
            if entry > 0.0 {
                let stop_pct = ((entry - stop) / entry * 100.0).abs();
                if stop_pct > config.max_stop_loss_pct {
                    issues.push(format!(
                        "stop loss percentage {:.1}% exceeds maximum {:.1}%",
                        stop_pct, config.max_stop_loss_pct
                    ));
                }
            }
        }
    }

    let is_valid = issues.is_empty();

    PickValidation {
        has_entry_price,
        has_stop_loss,
        has_target,
        risk_reward_ratio,
        has_catalyst,
        has_exit_strategy,
        is_valid,
        issues,
    }
}

/// Apply reasonable defaults for missing actionable fields.
pub(crate) fn apply_defaults(pick: &mut GeneratedStockPickItem, candidate: &EnrichedCandidate) {
    let current_price = candidate.price.or(candidate.market_snapshot.current_price);
    let atr = candidate.technical_snapshot.atr;

    // Default entry price: current price
    if pick.entry_price.is_none() {
        if let Some(price) = current_price {
            pick.entry_price = Some(format!("{:.2}", price));
        }
    }

    // Default stop loss: 2 * ATR below entry, or 5% below entry if ATR unavailable
    if pick.stop_loss.is_none() {
        if let Some(entry_str) = &pick.entry_price {
            if let Some(entry) = parse_price(entry_str) {
                let stop = if let Some(atr_val) = atr {
                    entry - 2.0 * atr_val
                } else {
                    entry * 0.95
                };
                pick.stop_loss = Some(format!("{:.2}", stop.max(0.01)));
            }
        }
    }

    // Default target price: 3:1 R/R from entry/stop
    if pick.target_price.is_none() {
        if let (Some(entry_str), Some(stop_str)) = (&pick.entry_price, &pick.stop_loss) {
            if let (Some(entry), Some(stop)) = (parse_price(entry_str), parse_price(stop_str)) {
                let risk = (entry - stop).abs();
                if risk > 0.0 {
                    let target = entry + 3.0 * risk;
                    pick.target_price = Some(format!("{:.2}", target));
                }
            }
        }
    }

    // Default holding period based on strategy
    if pick.holding_period.is_none() {
        pick.holding_period = Some("2-4 weeks".to_string());
    }

    // Default exit triggers
    if pick.exit_triggers.is_empty() {
        if let Some(stop_str) = &pick.stop_loss {
            pick.exit_triggers.push(format!("break below {}", stop_str));
        }
    }
}

/// Validate and enhance picks, rejecting those that fail quality gates.
pub(crate) fn validate_and_enhance_picks(
    picks: Vec<GeneratedStockPickItem>,
    candidates: &[EnrichedCandidate],
    config: &PickQualityGate,
) -> Vec<GeneratedStockPickItem> {
    picks
        .into_iter()
        .filter_map(|mut pick| {
            let candidate = candidates.iter().find(|c| c.symbol == pick.symbol)?;

            apply_defaults(&mut pick, candidate);

            let validation = validate_pick(&pick, candidate.price, config);

            if validation.is_valid {
                Some(pick)
            } else {
                tracing::warn!(
                    symbol = %pick.symbol,
                    issues = ?validation.issues,
                    "pick rejected by quality gate"
                );
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn make_pick(
        entry: Option<&str>,
        stop: Option<&str>,
        target: Option<&str>,
        catalysts: Vec<&str>,
        exit_triggers: Vec<&str>,
    ) -> GeneratedStockPickItem {
        GeneratedStockPickItem {
            symbol: "TEST".to_string(),
            confidence: Value::from(0.7),
            thesis: "Test thesis".to_string(),
            catalysts: catalysts.into_iter().map(String::from).collect(),
            risks: vec![],
            evidence_points: vec![],
            decision_reason_codes: vec![],
            data_gaps: vec![],
            entry_price: entry.map(String::from),
            stop_loss: stop.map(String::from),
            target_price: target.map(String::from),
            holding_period: None,
            exit_triggers: exit_triggers.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_validate_pick_valid() {
        let pick = make_pick(
            Some("100"),
            Some("95"),
            Some("115"),
            vec!["catalyst1"],
            vec!["break below 95"],
        );
        let config = PickQualityGate::default();
        let validation = validate_pick(&pick, Some(100.0), &config);
        assert!(validation.is_valid, "Expected valid, got: {:?}", validation.issues);
        assert!((validation.risk_reward_ratio - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_validate_pick_missing_fields() {
        let pick = make_pick(None, None, None, vec![], vec![]);
        let config = PickQualityGate::default();
        let validation = validate_pick(&pick, Some(100.0), &config);
        assert!(!validation.is_valid);
        assert!(validation.issues.iter().any(|i| i.contains("entry_price")));
        assert!(validation.issues.iter().any(|i| i.contains("stop_loss")));
        assert!(validation.issues.iter().any(|i| i.contains("target_price")));
    }

    #[test]
    fn test_validate_pick_stop_above_entry() {
        let pick = make_pick(
            Some("100"),
            Some("105"),
            Some("115"),
            vec!["catalyst1"],
            vec!["break below 105"],
        );
        let config = PickQualityGate::default();
        let validation = validate_pick(&pick, Some(100.0), &config);
        assert!(!validation.is_valid);
        assert!(validation.issues.iter().any(|i| i.contains("stop_loss must be below")));
    }

    #[test]
    fn test_validate_pick_low_rr() {
        let pick = make_pick(
            Some("100"),
            Some("95"),
            Some("102"),
            vec!["catalyst1"],
            vec!["break below 95"],
        );
        let config = PickQualityGate::default();
        let validation = validate_pick(&pick, Some(100.0), &config);
        assert!(!validation.is_valid);
        assert!(validation.issues.iter().any(|i| i.contains("risk/reward")));
    }
}

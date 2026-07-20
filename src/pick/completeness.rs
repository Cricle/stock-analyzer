use crate::StockPickItem;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub enum RationaleQuality {
    Strong,       // ≥10 words + data reference
    Adequate,     // ≥10 words
    Weak,         // 5-9 words + data reference
    Insufficient, // < 5 words
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CriticalFieldCompleteness {
    pub has_entry_with_rationale: bool,
    pub has_stop_with_rationale: bool,
    pub has_target_with_rationale: bool,
    pub has_catalysts_with_timing: bool,
    pub missing_fields: Vec<String>,
    pub score: i32,
}

fn assess_rationale_quality(rationale: &str) -> RationaleQuality {
    let word_count = rationale.split_whitespace().count();
    let has_data_ref = rationale.to_lowercase().contains("rsi")
        || rationale.to_lowercase().contains("macd")
        || rationale.to_lowercase().contains("pe")
        || rationale.to_lowercase().contains("price")
        || rationale.to_lowercase().contains("support")
        || rationale.to_lowercase().contains("resistance")
        || rationale.to_lowercase().contains("fibonacci")
        || rationale.to_lowercase().contains("measured")
        || rationale.chars().any(|c| c.is_numeric());

    match (word_count, has_data_ref) {
        (n, true) if n >= 10 => RationaleQuality::Strong,
        (n, _) if n >= 10 => RationaleQuality::Adequate,
        (n, true) if n >= 5 => RationaleQuality::Weak,
        _ => RationaleQuality::Insufficient,
    }
}

fn rationale_quality_score(quality: RationaleQuality) -> i32 {
    match quality {
        RationaleQuality::Strong => 5,
        RationaleQuality::Adequate => 3,
        RationaleQuality::Weak => 2,
        RationaleQuality::Insufficient => 0,
    }
}

pub fn score_critical_field_completeness(pick: &StockPickItem) -> CriticalFieldCompleteness {
    let mut score = 0;
    let mut missing = Vec::new();

    // Entry + rationale
    let has_entry = pick
        .entry_price
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty());
    let entry_rationale = pick.entry_rationale.as_deref().unwrap_or("");
    let entry_quality = assess_rationale_quality(entry_rationale);
    let has_entry_with_rationale = has_entry && entry_quality != RationaleQuality::Insufficient;

    if has_entry_with_rationale {
        score += rationale_quality_score(entry_quality);
    } else {
        missing.push("entry_rationale".to_string());
    }

    // Stop + rationale
    let has_stop = pick
        .stop_loss
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty());
    let stop_rationale = pick.stop_rationale.as_deref().unwrap_or("");
    let stop_quality = assess_rationale_quality(stop_rationale);
    let has_stop_with_rationale = has_stop && stop_quality != RationaleQuality::Insufficient;

    if has_stop_with_rationale {
        score += rationale_quality_score(stop_quality);
    } else {
        missing.push("stop_rationale".to_string());
    }

    // Target + rationale
    let has_target = pick
        .target_price
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty());
    let target_rationale = pick.target_rationale.as_deref().unwrap_or("");
    let target_quality = assess_rationale_quality(target_rationale);
    let has_target_with_rationale = has_target && target_quality != RationaleQuality::Insufficient;

    if has_target_with_rationale {
        score += rationale_quality_score(target_quality);
    } else {
        missing.push("target_rationale".to_string());
    }

    // Catalysts with timing
    let has_catalysts = !pick.catalysts.is_empty();
    let has_timing = has_catalysts
        && pick.catalysts.iter().any(|c| {
            let text = c.key.to_lowercase();
            text.contains("week")
                || text.contains("month")
                || text.contains("quarter")
                || text.contains("next")
                || text.contains("upcoming")
                || text.contains("soon")
        });
    let has_catalysts_with_timing = has_catalysts && has_timing;

    if has_catalysts_with_timing {
        score += 5;
    } else if has_catalysts {
        score += 1;
        missing.push("catalyst_timing".to_string());
    } else {
        missing.push("catalysts".to_string());
    }

    // Risks
    if pick.risks.len() >= 2 {
        score += 2;
    } else {
        missing.push("risks".to_string());
    }

    // Bonuses
    if pick.holding_period.is_some() {
        score += 1;
    }
    if !pick.exit_triggers.is_empty() {
        score += 1;
    }

    let score = score.min(20);

    CriticalFieldCompleteness {
        has_entry_with_rationale,
        has_stop_with_rationale,
        has_target_with_rationale,
        has_catalysts_with_timing,
        missing_fields: missing,
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pick_all_fields() -> StockPickItem {
        StockPickItem {
            entry_price: Some("150.00".to_string()),
            entry_rationale: Some("Entry at 150 based on RSI support and breakout confirmation at key resistance level".to_string()),
            stop_loss: Some("140.00".to_string()),
            stop_rationale: Some("Stop loss at 140 based on recent swing low and 7% risk tolerance".to_string()),
            target_price: Some("180.00".to_string()),
            target_rationale: Some("Target price 180 based on measured move and fibonacci extension levels".to_string()),
            catalysts: vec![
                crate::guide::I18nText::new("Earnings report next week"),
            ],
            risks: vec![
                crate::guide::I18nText::new("Market volatility"),
                crate::guide::I18nText::new("Regulatory concerns"),
            ],
            holding_period: Some("3-6 months".to_string()),
            exit_triggers: vec!["Break below support".to_string()],
            ..Default::default()
        }
    }

    fn create_test_pick_missing_rationales() -> StockPickItem {
        StockPickItem {
            entry_price: Some("150.00".to_string()),
            entry_rationale: Some("Buy".to_string()), // Insufficient
            stop_loss: Some("140.00".to_string()),
            stop_rationale: None, // Missing
            target_price: Some("180.00".to_string()),
            target_rationale: Some("Good upside".to_string()), // Insufficient
            catalysts: vec![],
            risks: vec![],
            holding_period: None,
            exit_triggers: vec![],
            ..Default::default()
        }
    }

    #[test]
    fn test_assess_rationale_quality_strong() {
        let rationale = "Entry at 150 based on RSI support and breakout confirmation";
        let quality = assess_rationale_quality(rationale);
        assert_eq!(quality, RationaleQuality::Strong);
    }

    #[test]
    fn test_score_critical_field_completeness_full() {
        let pick = create_test_pick_all_fields();
        let result = score_critical_field_completeness(&pick);
        assert_eq!(result.score, 20);
        assert!(result.missing_fields.is_empty());
    }

    #[test]
    fn test_score_critical_field_completeness_missing() {
        let pick = create_test_pick_missing_rationales();
        let result = score_critical_field_completeness(&pick);
        assert!(result.score < 20);
        assert!(!result.missing_fields.is_empty());
    }
}

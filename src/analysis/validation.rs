use serde::{Deserialize, Serialize};

/// Result of running all validators on an LLM output.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the recommendation contradicts technical indicators.
    pub consistency_flag: bool,
    /// Reason for consistency flag (empty if not flagged).
    pub consistency_reason: String,
    /// Whether outputs are uniform across stocks in a batch.
    pub uniformity_flag: bool,
    /// Percentage of fields that are identical across stocks.
    pub uniformity_pct: f64,
    /// Missing execution boundary fields.
    pub missing_boundary_fields: Vec<String>,
    /// Confidence adjustment from validation (negative = reduce).
    pub confidence_adjustment: i32,
    /// Action score adjustment from validation (negative = reduce).
    pub action_adjustment: i32,
}

impl ValidationResult {
    /// Returns true if any validator flagged an issue.
    pub fn has_issues(&self) -> bool {
        self.consistency_flag || self.uniformity_flag || !self.missing_boundary_fields.is_empty()
    }
}

/// Check if recommendation contradicts technical indicators.
pub fn check_consistency(
    recommendation: &str,
    rsi: f64,
    macd_signal: &str,
) -> ValidationResult {
    let mut result = ValidationResult::default();
    let rec_lower = recommendation.to_lowercase();
    let is_sell = rec_lower.contains("sell") || rec_lower.contains("underweight");
    let is_buy = rec_lower.contains("buy") || rec_lower.contains("overweight");
    let macd_bullish = macd_signal.contains("bullish");
    let macd_bearish = macd_signal.contains("bearish");

    // Strong contradiction: sell with oversold RSI + bullish MACD
    if is_sell && rsi < 30.0 && macd_bullish {
        result.consistency_flag = true;
        result.consistency_reason = format!(
            "Sell/Underweight but RSI={:.1} (oversold) + MACD={}",
            rsi, macd_signal
        );
        result.confidence_adjustment = -15;
    }
    // Moderate contradiction: sell with near-oversold RSI + bullish MACD
    else if is_sell && rsi < 40.0 && macd_bullish {
        result.consistency_flag = true;
        result.consistency_reason = format!(
            "Sell/Underweight but RSI={:.1} (near-oversold) + MACD={}",
            rsi, macd_signal
        );
        result.confidence_adjustment = -10;
    }
    // Mild contradiction: sell with neutral RSI + bullish MACD (common pattern)
    else if is_sell && rsi < 45.0 && macd_bullish {
        result.consistency_flag = true;
        result.consistency_reason = format!(
            "Sell/Underweight but RSI={:.1} (neutral-low) + MACD={}",
            rsi, macd_signal
        );
        result.confidence_adjustment = -6;
    }
    // Strong contradiction: buy with overbought RSI + bearish MACD
    else if is_buy && rsi > 70.0 && macd_bearish {
        result.consistency_flag = true;
        result.consistency_reason = format!(
            "Buy/Overweight but RSI={:.1} (overbought) + MACD={}",
            rsi, macd_signal
        );
        result.confidence_adjustment = -15;
    }
    // Moderate contradiction: buy with near-overbought RSI + bearish MACD
    else if is_buy && rsi > 60.0 && macd_bearish {
        result.consistency_flag = true;
        result.consistency_reason = format!(
            "Buy/Overweight but RSI={:.1} (near-overbought) + MACD={}",
            rsi, macd_signal
        );
        result.confidence_adjustment = -10;
    }

    result
}

/// Check if recommendation is consistent with price position.
pub fn check_price_position(
    recommendation: &str,
    distance_to_low_pct: f64,
    distance_to_high_pct: f64,
) -> ValidationResult {
    let mut result = ValidationResult::default();
    let rec_lower = recommendation.to_lowercase();
    let is_sell = rec_lower.contains("sell") || rec_lower.contains("underweight");
    let is_buy = rec_lower.contains("buy") || rec_lower.contains("overweight");

    // Contradiction: sell when price is already near 60-day low
    if is_sell && distance_to_low_pct < 5.0 {
        result.consistency_flag = true;
        result.consistency_reason = format!(
            "Sell/Underweight but price is {:.1}% above 60-day low (downside limited)",
            distance_to_low_pct
        );
        result.confidence_adjustment = -8;
    }
    // Contradiction: buy when price is already near 60-day high
    if is_buy && distance_to_high_pct < 5.0 {
        result.consistency_flag = true;
        result.consistency_reason = format!(
            "Buy/Overweight but price is {:.1}% below 60-day high (upside limited)",
            distance_to_high_pct
        );
        result.confidence_adjustment = -8;
    }

    result
}

/// Check if outputs are uniform across stocks in a batch.
/// Each tuple is (symbol, entry_price, stop_loss, position_sizing, time_horizon).
pub fn check_uniformity(
    stocks: &[(&str, &str, &str, &str, &str)],
) -> ValidationResult {
    let mut result = ValidationResult::default();
    if stocks.len() < 2 {
        return result;
    }

    let total_fields = stocks.len() * 4; // 4 fields per stock
    let mut identical_fields = 0;

    // Compare each field across stocks
    for field_idx in 0..4 {
        let values: Vec<&str> = stocks.iter().map(|s| match field_idx {
            0 => s.1,
            1 => s.2,
            2 => s.3,
            3 => s.4,
            _ => unreachable!(),
        }).collect();

        let first = values[0];
        if values.iter().all(|v| *v == first) && !first.is_empty() {
            identical_fields += stocks.len();
        }
    }

    let uniformity_pct = (identical_fields as f64 / total_fields as f64) * 100.0;
    result.uniformity_pct = uniformity_pct;

    if uniformity_pct > 70.0 {
        result.uniformity_flag = true;
        result.action_adjustment = -18;
    }

    result
}

/// Check if required execution boundary fields are present.
pub fn check_execution_boundary(
    recommendation: &str,
    entry_price: &str,
    stop_loss: &str,
    confirmation_level: &str,
    invalidation_level: &str,
) -> ValidationResult {
    let mut result = ValidationResult::default();
    let rec_lower = recommendation.to_lowercase();
    let is_directional = rec_lower.contains("buy")
        || rec_lower.contains("sell")
        || rec_lower.contains("overweight")
        || rec_lower.contains("underweight");

    if !is_directional {
        return result;
    }

    if entry_price.trim().is_empty() {
        result.missing_boundary_fields.push("entry_price".to_string());
    }
    if stop_loss.trim().is_empty() {
        result.missing_boundary_fields.push("stop_loss".to_string());
    }
    if confirmation_level.trim().is_empty() && invalidation_level.trim().is_empty() {
        result.missing_boundary_fields.push("confirmation_level".to_string());
        result.missing_boundary_fields.push("invalidation_level".to_string());
    }

    result
}

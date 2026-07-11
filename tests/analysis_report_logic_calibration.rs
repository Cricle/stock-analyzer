use stock_analyzer::analysis::{MemoryContextSnapshot, Rating};
use stock_analyzer::analysis::{derive_calibration_bias, fallback_sizing_reference};

#[test]
fn derive_calibration_bias_misaligned() {
    let ctx = MemoryContextSnapshot::default();
    let bias = derive_calibration_bias(&ctx, false, true, false);
    assert_eq!(bias.direction.key, "negative");
    assert_eq!(bias.magnitude.key, "high");
}

#[test]
fn derive_calibration_bias_threshold_tightened() {
    let ctx = MemoryContextSnapshot::default();
    let bias = derive_calibration_bias(&ctx, true, false, false);
    assert_eq!(bias.direction.key, "negative");
    assert_eq!(bias.magnitude.key, "medium");
}

#[test]
fn derive_calibration_bias_positive_support() {
    let ctx = MemoryContextSnapshot {
        setup_resolved_match_count: 5,
        setup_match_hit_rate: 0.8,
        setup_match_avg_alpha_return: 0.05,
        ..Default::default()
    };
    let bias = derive_calibration_bias(&ctx, false, false, true);
    assert_eq!(bias.direction.key, "positive");
    assert_eq!(bias.magnitude.key, "low");
}

#[test]
fn derive_calibration_bias_neutral() {
    let ctx = MemoryContextSnapshot::default();
    let bias = derive_calibration_bias(&ctx, false, false, false);
    assert_eq!(bias.direction.key, "neutral");
    assert_eq!(bias.magnitude.key, "low");
}

#[test]
fn derive_calibration_bias_priority_misaligned_over_threshold() {
    // misaligned takes priority over threshold_tightened
    let ctx = MemoryContextSnapshot::default();
    let bias = derive_calibration_bias(&ctx, true, true, false);
    assert_eq!(bias.direction.key, "negative");
    assert_eq!(bias.magnitude.key, "high");
}

#[test]
fn fallback_sizing_reference_blocker_present() {
    let result = fallback_sizing_reference("30%", &Rating::Buy, true);
    assert_eq!(result.key, "sizing_reference_blockers");
}

#[test]
fn fallback_sizing_reference_existing_plan() {
    let result = fallback_sizing_reference("50%", &Rating::Buy, false);
    assert_eq!(result.key, "sizing_reference_from_plan");
}

#[test]
fn fallback_sizing_reference_empty_bullish() {
    let result = fallback_sizing_reference("", &Rating::Buy, false);
    assert_eq!(result.key, "sizing_reference_bullish");
}

#[test]
fn fallback_sizing_reference_empty_bearish() {
    let result = fallback_sizing_reference("", &Rating::Sell, false);
    assert_eq!(result.key, "sizing_reference_bearish");
}

#[test]
fn fallback_sizing_reference_empty_neutral() {
    let result = fallback_sizing_reference("", &Rating::Hold, false);
    assert_eq!(result.key, "sizing_reference_neutral");
}

#[test]
fn fallback_sizing_reference_whitespace_only() {
    let result = fallback_sizing_reference("  ", &Rating::Buy, false);
    // Whitespace-only is treated as empty
    assert_eq!(result.key, "sizing_reference_bullish");
}

#[test]
fn fallback_sizing_reference_blocker_overrides_plan() {
    let result = fallback_sizing_reference("30%", &Rating::Buy, true);
    assert_eq!(result.key, "sizing_reference_blockers");
}

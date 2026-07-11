use stock_analyzer::value_utils::{clamp_probability, normalize_probability, normalize_value};
use serde_json::json;

// --- normalize_probability ---

#[test]
fn normalize_probability_number() {
    assert!((normalize_probability(&json!(0.75)) - 0.75).abs() < 0.01);
}

#[test]
fn normalize_probability_string_decimal() {
    assert!((normalize_probability(&json!("0.65")) - 0.65).abs() < 0.01);
}

#[test]
fn normalize_probability_string_percent() {
    assert!((normalize_probability(&json!("75%")) - 0.75).abs() < 0.01);
}

#[test]
fn normalize_probability_string_over_100() {
    assert!((normalize_probability(&json!("150")) - 1.0).abs() < 0.01);
}

#[test]
fn normalize_probability_array() {
    assert!((normalize_probability(&json!([0.0, 0.8, 0.5])) - 0.8).abs() < 0.01);
}

#[test]
fn normalize_probability_object_with_value_key() {
    assert!((normalize_probability(&json!({"value": 0.9})) - 0.9).abs() < 0.01);
}

#[test]
fn normalize_probability_object_with_probability_key() {
    assert!((normalize_probability(&json!({"probability": 0.55})) - 0.55).abs() < 0.01);
}

#[test]
fn normalize_probability_null() {
    assert_eq!(normalize_probability(&json!(null)), 0.0);
}

#[test]
fn normalize_probability_bool() {
    assert_eq!(normalize_probability(&json!(true)), 0.0);
}

#[test]
fn normalize_probability_clamped_below() {
    assert_eq!(normalize_probability(&json!(-0.5)), 0.0);
}

// --- normalize_value ---

#[test]
fn normalize_value_null() {
    assert_eq!(normalize_value(&json!(null)), "");
}

#[test]
fn normalize_value_string() {
    assert_eq!(normalize_value(&json!("hello")), "hello");
}

#[test]
fn normalize_value_string_trimmed() {
    assert_eq!(normalize_value(&json!("  hello  ")), "hello");
}

#[test]
fn normalize_value_number() {
    assert_eq!(normalize_value(&json!(42)), "42");
}

#[test]
fn normalize_value_bool() {
    assert_eq!(normalize_value(&json!(true)), "true");
}

#[test]
fn normalize_value_array() {
    assert_eq!(normalize_value(&json!(["a", "b", "c"])), "a\nb\nc");
}

#[test]
fn normalize_value_object() {
    let result = normalize_value(&json!({"key": "value"}));
    assert_eq!(result, "key: value");
}

#[test]
fn normalize_value_object_skips_empty() {
    let result = normalize_value(&json!({"key": ""}));
    assert_eq!(result, "");
}

// --- clamp_probability ---

#[test]
fn clamp_probability_in_range() {
    assert!((clamp_probability(0.5) - 0.5).abs() < 0.01);
}

#[test]
fn clamp_probability_above() {
    assert!((clamp_probability(1.5) - 1.0).abs() < 0.01);
}

#[test]
fn clamp_probability_below() {
    assert!((clamp_probability(-0.5) - 0.0).abs() < 0.01);
}

#[test]
fn normalize_probability_object_with_score_key() {
    assert!((normalize_probability(&json!({"score": 0.85})) - 0.85).abs() < 0.01);
}

#[test]
fn normalize_probability_object_with_confidence_key() {
    assert!((normalize_probability(&json!({"confidence": 0.7})) - 0.7).abs() < 0.01);
}

#[test]
fn normalize_probability_object_with_up_key() {
    assert!((normalize_probability(&json!({"up": 0.6})) - 0.6).abs() < 0.01);
}

#[test]
fn normalize_probability_object_with_down_key() {
    assert!((normalize_probability(&json!({"down": 0.3})) - 0.3).abs() < 0.01);
}

#[test]
fn normalize_probability_object_with_sideways_key() {
    assert!((normalize_probability(&json!({"sideways": 0.4})) - 0.4).abs() < 0.01);
}

#[test]
fn normalize_probability_object_falls_back_to_values() {
    assert!((normalize_probability(&json!({"other": 0.55})) - 0.55).abs() < 0.01);
}

#[test]
fn normalize_probability_object_all_zeros() {
    assert_eq!(
        normalize_probability(&json!({"value": 0.0, "score": 0.0})),
        0.0
    );
}

#[test]
fn normalize_probability_string_with_spaces() {
    assert!((normalize_probability(&json!("  0.75  ")) - 0.75).abs() < 0.01);
}

#[test]
fn normalize_probability_string_percent_with_spaces() {
    assert!((normalize_probability(&json!("  80%  ")) - 0.80).abs() < 0.01);
}

#[test]
fn normalize_probability_array_all_zeros() {
    assert_eq!(normalize_probability(&json!([0.0, 0.0, 0.0])), 0.0);
}

#[test]
fn normalize_value_array_with_nulls() {
    assert_eq!(normalize_value(&json!(["a", null, "b"])), "a\nb");
}

#[test]
fn normalize_value_object_nested() {
    let result = normalize_value(&json!({"k": {"nested": "v"}}));
    assert!(result.contains("nested: v"));
}

#[test]
fn normalize_inline_value_object_in_array() {
    let result = normalize_value(&json!([{"k": "v"}]));
    assert_eq!(result, "k: v");
}

#[test]
fn normalize_value_empty_array() {
    assert_eq!(normalize_value(&json!([])), "");
}

#[test]
fn normalize_value_empty_object() {
    assert_eq!(normalize_value(&json!({})), "");
}

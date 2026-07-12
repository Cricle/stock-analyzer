#![allow(clippy::approx_constant)]

use serde_json::json;
use stock_analyzer::llm::parse::*;

// --- text_or_default ---

#[test]
fn text_or_default_some() {
    assert_eq!(text_or_default(Some(json!("hello")), "fallback"), "hello");
}

#[test]
fn text_or_default_none() {
    assert_eq!(text_or_default(None, "fallback"), "fallback");
}

#[test]
fn text_or_default_empty_string() {
    assert_eq!(text_or_default(Some(json!("")), "fallback"), "fallback");
}

#[test]
fn text_or_default_number() {
    assert_eq!(text_or_default(Some(json!(42)), "fallback"), "42");
}

// --- first_non_empty ---

#[test]
fn first_non_empty_first_match() {
    let a = json!("first");
    let b = json!("second");
    assert_eq!(first_non_empty(&[Some(&a), Some(&b)], "default"), "first");
}

#[test]
fn first_non_empty_skip_empty() {
    let a = json!("");
    let b = json!("second");
    assert_eq!(first_non_empty(&[Some(&a), Some(&b)], "default"), "second");
}

#[test]
fn first_non_empty_all_none() {
    assert_eq!(first_non_empty(&[None, None], "default"), "default");
}

// --- string_list_or_default ---

#[test]
fn string_list_or_default_array() {
    let val = json!(["a", "b", "c"]);
    assert_eq!(
        string_list_or_default(Some(val), &["x"]),
        vec!["a", "b", "c"]
    );
}

#[test]
fn string_list_or_default_empty_array() {
    let val = json!([]);
    assert_eq!(string_list_or_default(Some(val), &["x"]), vec!["x"]);
}

#[test]
fn string_list_or_default_none() {
    assert_eq!(string_list_or_default(None, &["x", "y"]), vec!["x", "y"]);
}

#[test]
fn string_list_or_default_string() {
    let val = json!("a\nb\nc");
    assert_eq!(
        string_list_or_default(Some(val), &["x"]),
        vec!["a", "b", "c"]
    );
}

// --- normalize_probability ---

#[test]
fn normalize_probability_number() {
    assert!((normalize_probability(&json!(0.75)) - 0.75).abs() < 0.01);
}

#[test]
fn normalize_probability_percent_string() {
    assert!((normalize_probability(&json!("75%")) - 0.75).abs() < 0.01);
}

#[test]
fn normalize_probability_decimal_string() {
    assert!((normalize_probability(&json!("0.65")) - 0.65).abs() < 0.01);
}

#[test]
fn normalize_probability_clamped() {
    assert_eq!(normalize_probability(&json!(1.5)), 1.0);
}

#[test]
fn normalize_probability_null() {
    assert_eq!(normalize_probability(&json!(null)), 0.0);
}

// --- normalize_numeric ---

#[test]
fn normalize_numeric_number() {
    assert_eq!(normalize_numeric(&json!(42.0)), Some(42.0));
}

#[test]
fn normalize_numeric_string() {
    assert_eq!(normalize_numeric(&json!("3.14")), Some(3.14));
}

#[test]
fn normalize_numeric_string_with_text() {
    assert_eq!(normalize_numeric(&json!("价格: 15.5 元")), Some(15.5));
}

#[test]
fn normalize_numeric_null() {
    assert_eq!(normalize_numeric(&json!(null)), None);
}

#[test]
fn normalize_numeric_array() {
    let val = json!(["abc", "5.5", "xyz"]);
    assert_eq!(normalize_numeric(&val), Some(5.5));
}

#[test]
fn normalize_numeric_object() {
    let val = json!({"value": 10.0, "name": "test"});
    assert_eq!(normalize_numeric(&val), Some(10.0));
}

// --- normalize_probability_triplet ---

#[test]
fn normalize_probability_triplet_balanced() {
    let (up, down, sideways) = normalize_probability_triplet(&json!(0.5), &json!(0.3), &json!(0.2));
    assert!((up - 0.5).abs() < 0.01);
    assert!((down - 0.3).abs() < 0.01);
    assert!((sideways - 0.2).abs() < 0.01);
}

#[test]
fn normalize_probability_triplet_unbalanced() {
    let (up, down, sideways) = normalize_probability_triplet(&json!(50), &json!(30), &json!(20));
    let total = up + down + sideways;
    assert!((total - 1.0).abs() < 0.01);
}

#[test]
fn normalize_probability_triplet_all_zero() {
    let (up, down, sideways) = normalize_probability_triplet(&json!(0), &json!(0), &json!(0));
    assert!((up - 0.33).abs() < 0.01);
    assert!((down - 0.33).abs() < 0.01);
    assert!((sideways - 0.34).abs() < 0.01);
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

// --- parse_first_numeric_token ---

#[test]
fn parse_first_numeric_token_simple() {
    assert_eq!(parse_first_numeric_token("42"), Some(42.0));
}

#[test]
fn parse_first_numeric_token_decimal() {
    assert_eq!(parse_first_numeric_token("3.14"), Some(3.14));
}

#[test]
fn parse_first_numeric_token_with_text() {
    assert_eq!(parse_first_numeric_token("价格: 15.5 元"), Some(15.5));
}

#[test]
fn parse_first_numeric_token_empty() {
    assert_eq!(parse_first_numeric_token(""), None);
}

#[test]
fn parse_first_numeric_token_no_number() {
    assert_eq!(parse_first_numeric_token("no numbers here"), None);
}

#[test]
fn parse_first_numeric_token_ma_period_skipped() {
    assert_eq!(parse_first_numeric_token("200日均线"), None);
}

// --- is_default_text ---

#[test]
fn is_default_text_valid() {
    assert!(is_default_text("模型未返回该角色摘要。"));
    assert!(is_default_text("模型未返回交易员计划。"));
}

#[test]
fn is_default_text_invalid() {
    assert!(!is_default_text("normal text"));
    assert!(!is_default_text(""));
}

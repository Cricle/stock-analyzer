use serde_json::Value;
use stock_analyzer::llm::parse::{
    decode_json_string_literal, extract_json_value_before_known_field,
    extract_relaxed_json_string_field, extract_simple_json_string_field, find_json_string_end,
    find_json_value_end, normalize_relaxed_json_string, skip_json_whitespace,
};

// --- skip_json_whitespace ---

#[test]
fn skip_whitespace_no_whitespace() {
    assert_eq!(skip_json_whitespace("abc", 0), 0);
}

#[test]
fn skip_whitespace_with_spaces() {
    assert_eq!(skip_json_whitespace("   abc", 0), 3);
}

#[test]
fn skip_whitespace_with_mixed() {
    assert_eq!(skip_json_whitespace("\t\n\r abc", 0), 4);
}

#[test]
fn skip_whitespace_at_end() {
    assert_eq!(skip_json_whitespace("   ", 0), 3);
}

#[test]
fn skip_whitespace_from_offset() {
    assert_eq!(skip_json_whitespace("a   b", 1), 4);
}

// --- find_json_string_end ---

#[test]
fn find_string_end_simple() {
    assert_eq!(find_json_string_end(r#""hello""#, 0), Some(6));
}

#[test]
fn find_string_end_empty() {
    assert_eq!(find_json_string_end(r#""""#, 0), Some(1));
}

#[test]
fn find_string_end_with_escape() {
    assert_eq!(find_json_string_end(r#""he\"llo""#, 0), Some(8));
}

#[test]
fn find_string_end_with_backslash_escape() {
    assert_eq!(find_json_string_end(r#""path\\dir""#, 0), Some(10));
}

#[test]
fn find_string_end_not_string() {
    assert_eq!(find_json_string_end("abc", 0), None);
}

#[test]
fn find_string_end_unterminated() {
    assert_eq!(find_json_string_end(r#""unterminated"#, 0), None);
}

// --- find_json_value_end ---

#[test]
fn find_value_end_string() {
    assert_eq!(find_json_value_end(r#""hello""#, 0), Some(6));
}

#[test]
fn find_value_end_object() {
    assert_eq!(find_json_value_end(r#"{"a":1}"#, 0), Some(6));
}

#[test]
fn find_value_end_array() {
    assert_eq!(find_json_value_end("[1,2,3]", 0), Some(6));
}

#[test]
fn find_value_end_number() {
    assert_eq!(find_json_value_end("42,", 0), Some(1));
}

#[test]
fn find_value_end_boolean() {
    assert_eq!(find_json_value_end("true,", 0), Some(3));
}

#[test]
fn find_value_end_null() {
    assert_eq!(find_json_value_end("null,", 0), Some(3));
}

#[test]
fn find_value_end_nested_object() {
    let input = r#"{"a":{"b":2}}"#;
    assert_eq!(find_json_value_end(input, 0), Some(12));
}

// --- decode_json_string_literal ---

#[test]
fn decode_simple_string() {
    assert_eq!(decode_json_string_literal(r#""hello""#).unwrap(), "hello");
}

#[test]
fn decode_string_with_escapes() {
    assert_eq!(
        decode_json_string_literal(r#""hello\nworld""#).unwrap(),
        "hello\nworld"
    );
}

#[test]
fn decode_invalid_literal() {
    assert!(decode_json_string_literal("not json").is_err());
}

// --- extract_simple_json_string_field ---

#[test]
fn extract_simple_field() {
    let input = r#"{"name":"Alice","age":30}"#;
    assert_eq!(
        extract_simple_json_string_field(input, "name"),
        Some("Alice".to_string())
    );
}

#[test]
fn extract_simple_field_with_spaces() {
    let input = r#"{"name" :  "Bob"  }"#;
    assert_eq!(
        extract_simple_json_string_field(input, "name"),
        Some("Bob".to_string())
    );
}

#[test]
fn extract_simple_field_missing() {
    let input = r#"{"age":30}"#;
    assert_eq!(extract_simple_json_string_field(input, "name"), None);
}

#[test]
fn extract_simple_field_not_string_value() {
    let input = r#"{"count":42}"#;
    assert_eq!(extract_simple_json_string_field(input, "count"), None);
}

// --- normalize_relaxed_json_string ---

#[test]
fn normalize_escapes_newline() {
    assert_eq!(
        normalize_relaxed_json_string("hello\\nworld"),
        "hello\nworld"
    );
}

#[test]
fn normalize_escapes_tab() {
    assert_eq!(normalize_relaxed_json_string("a\\tb"), "a\tb");
}

#[test]
fn normalize_escapes_carriage_return() {
    assert_eq!(normalize_relaxed_json_string("a\\rb"), "a\rb");
}

#[test]
fn normalize_escapes_quote() {
    let input = r#"value with \"quotes\" inside"#;
    let result = normalize_relaxed_json_string(input);
    assert!(result.contains("value with"));
    assert!(result.contains("quotes"));
}

#[test]
fn normalize_combined_escapes() {
    let input = "hello\\nworld\\tnow";
    let result = normalize_relaxed_json_string(input);
    assert_eq!(result, "hello\nworld\tnow");
}

#[test]
fn normalize_trims_trailing_quote() {
    assert_eq!(normalize_relaxed_json_string("hello\""), "hello");
}

#[test]
fn normalize_trims_whitespace() {
    assert_eq!(normalize_relaxed_json_string("  hello  "), "hello");
}

// --- extract_relaxed_json_string_field ---

#[test]
fn extract_relaxed_field_simple() {
    let input = r#"{"summary":"hello world","next_field":"val"}"#;
    let result = extract_relaxed_json_string_field(input, "summary", &["next_field"]);
    assert_eq!(result, Some("hello world".to_string()));
}

#[test]
fn extract_relaxed_field_missing_field() {
    let input = r#"{"other":"value"}"#;
    let result = extract_relaxed_json_string_field(input, "summary", &["other"]);
    assert_eq!(result, None);
}

#[test]
fn extract_relaxed_field_no_next_field_match() {
    let input = r#"{"summary":"hello","unrelated":"val"}"#;
    let result = extract_relaxed_json_string_field(input, "summary", &["nonexistent"]);
    assert_eq!(result, None);
}

// --- extract_json_value_before_known_field ---

#[test]
fn extract_value_before_field_string() {
    let input = r#"{"name":"Alice","age":30}"#;
    let result = extract_json_value_before_known_field(input, "name", &["age"]);
    assert_eq!(result, Some(Value::String("Alice".into())));
}

#[test]
fn extract_value_before_field_number() {
    let input = r#"{"count":42,"name":"test"}"#;
    let result = extract_json_value_before_known_field(input, "count", &["name"]);
    assert_eq!(result, Some(Value::Number(serde_json::Number::from(42))));
}

#[test]
fn extract_value_before_field_object() {
    let input = r#"{"data":{"x":1},"name":"test"}"#;
    let result = extract_json_value_before_known_field(input, "data", &["name"]);
    assert!(result.is_some());
    assert!(result.unwrap().is_object());
}

#[test]
fn extract_value_before_field_missing_next() {
    let input = r#"{"name":"Alice"}"#;
    let result = extract_json_value_before_known_field(input, "name", &["nonexistent"]);
    assert_eq!(result, None);
}

#[test]
fn extract_value_before_field_empty_next_fields() {
    let input = r#"{"name":"Alice"}"#;
    let result = extract_json_value_before_known_field(input, "name", &[]);
    assert_eq!(result, Some(Value::String("Alice".into())));
}

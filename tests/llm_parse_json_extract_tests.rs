use sa::llm::parse::{
    repair_bracket_confusion, repair_common_malformed_json_variants,
    slice_first_complete_json_value, slice_outer_json_object, strip_code_fence,
};

// --- strip_code_fence ---

#[test]
fn strip_code_fence_json_block() {
    let input = "```json\n{\"key\": \"value\"}\n```";
    assert_eq!(strip_code_fence(input), Some("{\"key\": \"value\"}"));
}

#[test]
fn strip_code_fence_plain_block() {
    let input = "```\nhello world\n```";
    assert_eq!(strip_code_fence(input), Some("hello world"));
}

#[test]
fn strip_code_fence_no_fence() {
    assert_eq!(strip_code_fence("no fence here"), None);
}

#[test]
fn strip_code_fence_no_newline_after_backticks() {
    assert_eq!(strip_code_fence("```json"), None);
}

#[test]
fn strip_code_fence_closing_fence_trimmed() {
    let input = "```\n  data  \n```";
    assert_eq!(strip_code_fence(input), Some("data"));
}

// --- slice_outer_json_object ---

#[test]
fn slice_outer_json_object_basic() {
    let input = "prefix {\"a\":1} suffix";
    assert_eq!(slice_outer_json_object(input), Some("{\"a\":1}"));
}

#[test]
fn slice_outer_json_object_nested() {
    let input = "text {\"a\":{\"b\":2}} end";
    assert_eq!(slice_outer_json_object(input), Some("{\"a\":{\"b\":2}}"));
}

#[test]
fn slice_outer_json_object_no_braces() {
    assert_eq!(slice_outer_json_object("no braces"), None);
}

#[test]
fn slice_outer_json_object_only_close() {
    assert_eq!(slice_outer_json_object("only } here"), None);
}

// --- slice_first_complete_json_value ---

#[test]
fn slice_first_json_object() {
    let input = "blah {\"x\": 1} trailing";
    assert_eq!(slice_first_complete_json_value(input), Some("{\"x\": 1}"));
}

#[test]
fn slice_first_json_array() {
    let input = "[1, 2, 3]";
    assert_eq!(slice_first_complete_json_value(input), Some("[1, 2, 3]"));
}

#[test]
fn slice_first_json_string() {
    let input = "  \"hello\"  ";
    assert_eq!(slice_first_complete_json_value(input), Some("\"hello\""));
}

#[test]
fn slice_first_json_number() {
    let input = "  42  ";
    assert_eq!(slice_first_complete_json_value(input), Some("42"));
}

#[test]
fn slice_first_json_nested_object_in_array() {
    let input = "[{\"a\":1},{\"b\":2}]";
    assert_eq!(
        slice_first_complete_json_value(input),
        Some("[{\"a\":1},{\"b\":2}]")
    );
}

#[test]
fn slice_first_json_no_value() {
    assert_eq!(slice_first_complete_json_value(""), None);
    assert_eq!(slice_first_complete_json_value("   "), None);
}

// --- repair_bracket_confusion ---

#[test]
fn repair_bracket_confusion_fixes_object() {
    let input = r#"{"key": ["inner_key": "value"]}"#;
    let result = repair_bracket_confusion(input).expect("should repair");
    assert!(result.contains(r#"{"inner_key": "value"}"#));
}

#[test]
fn repair_bracket_confusion_no_change_needed() {
    let input = r#"{"key": [1, 2, 3]}"#;
    assert_eq!(repair_bracket_confusion(input), None);
}

// --- repair_common_malformed_json_variants ---

#[test]
fn repair_common_malformed_no_change() {
    let input = r#"{"valid": "json"}"#;
    assert_eq!(repair_common_malformed_json_variants(input), None);
}

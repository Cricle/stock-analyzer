use serde_json::Value;
pub(crate) fn skip_json_whitespace(content: &str, mut index: usize) -> usize {
    let bytes = content.as_bytes();
    while let Some(byte) = bytes.get(index) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        index += 1;
    }
    index
}

pub(crate) fn find_json_string_end(content: &str, start: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    if bytes.get(start).copied()? != b'"' {
        return None;
    }

    let mut index = start + 1;
    let mut escaped = false;
    while let Some(byte) = bytes.get(index) {
        if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == b'"' {
            return Some(index);
        }
        index += 1;
    }

    None
}

pub(crate) fn find_json_value_end(content: &str, start: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    match bytes.get(start).copied()? {
        b'"' => find_json_string_end(content, start),
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut index = start;
            let mut in_string = false;
            let mut escaped = false;
            while let Some(byte) = bytes.get(index) {
                if in_string {
                    if escaped {
                        escaped = false;
                    } else if *byte == b'\\' {
                        escaped = true;
                    } else if *byte == b'"' {
                        in_string = false;
                    }
                    index += 1;
                    continue;
                }

                match *byte {
                    b'"' => in_string = true,
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Some(index);
                        }
                    }
                    _ => {}
                }
                index += 1;
            }
            None
        }
        _ => {
            let mut index = start;
            while let Some(byte) = bytes.get(index) {
                if matches!(*byte, b',' | b'}' | b']') {
                    return index.checked_sub(1);
                }
                if byte.is_ascii_whitespace() {
                    let next = skip_json_whitespace(content, index);
                    if matches!(
                        bytes.get(next).copied(),
                        Some(b',') | Some(b'}') | Some(b']')
                    ) {
                        return index.checked_sub(1);
                    }
                }
                index += 1;
            }
            Some(content.len().saturating_sub(1))
        }
    }
}

pub(crate) fn decode_json_string_literal(literal: &str) -> anyhow::Result<String> {
    serde_json::from_str::<String>(literal).map_err(Into::into)
}

pub(crate) fn extract_simple_json_string_field(content: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let start = content.find(&key)?;
    let after_key = start + key.len();
    let colon = content[after_key..].find(':')? + after_key;
    let value_start = skip_json_whitespace(content, colon + 1);
    let value_end = find_json_string_end(content, value_start)?;
    decode_json_string_literal(&content[value_start..=value_end]).ok()
}

pub(crate) fn extract_relaxed_json_string_field(
    content: &str,
    field: &str,
    next_fields: &[&str],
) -> Option<String> {
    let key = format!("\"{field}\"");
    let start = content.find(&key)?;
    let after_key = start + key.len();
    let colon = content[after_key..].find(':')? + after_key;
    let value_start = skip_json_whitespace(content, colon + 1);
    if content.as_bytes().get(value_start).copied()? != b'"' {
        return None;
    }
    let raw_start = value_start + 1;
    let marker = next_fields
        .iter()
        .filter_map(|next| {
            content[raw_start..]
                .find(&format!(",\"{next}\""))
                .map(|offset| raw_start + offset)
        })
        .min()?;
    let raw = content[raw_start..marker].trim_end();
    Some(normalize_relaxed_json_string(raw))
}

pub(crate) fn extract_json_value_before_known_field(
    content: &str,
    field: &str,
    next_fields: &[&str],
) -> Option<Value> {
    let key = format!("\"{field}\"");
    let start = content.find(&key)?;
    let after_key = start + key.len();
    let colon = content[after_key..].find(':')? + after_key;
    let value_start = skip_json_whitespace(content, colon + 1);

    let value_slice = if next_fields.is_empty() {
        let end = find_json_value_end(content, value_start)?;
        content[value_start..=end]
            .trim()
            .trim_end_matches(',')
            .trim()
    } else {
        let marker = next_fields
            .iter()
            .filter_map(|next| {
                content[value_start..]
                    .find(&format!(",\"{next}\""))
                    .map(|offset| value_start + offset)
            })
            .min()?;
        content[value_start..marker]
            .trim()
            .trim_end_matches(',')
            .trim()
    };

    serde_json::from_str::<Value>(value_slice).ok()
}

fn normalize_relaxed_json_string(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('"').trim_end();
    trimmed
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\r", "\r")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

#[cfg(test)]
mod json_string_tests {
    use super::super::*;

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
        // "42," -> number ends at index 1 (before comma)
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
        assert_eq!(normalize_relaxed_json_string("hello\\nworld"), "hello\nworld");
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
        // Input has embedded \" which gets replaced with "
        // Note: trim_end_matches('"') runs first, then replace
        let input = r#"value with \"quotes\" inside"#;
        let result = normalize_relaxed_json_string(input);
        assert!(result.contains("value with"));
        assert!(result.contains("quotes"));
    }

    #[test]
    fn normalize_combined_escapes() {
        // Test that \n and \t are both replaced
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
        assert_eq!(result, Some(serde_json::Value::String("Alice".into())));
    }

    #[test]
    fn extract_value_before_field_number() {
        let input = r#"{"count":42,"name":"test"}"#;
        let result = extract_json_value_before_known_field(input, "count", &["name"]);
        assert_eq!(result, Some(serde_json::Value::Number(serde_json::Number::from(42))));
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
        assert_eq!(result, Some(serde_json::Value::String("Alice".into())));
    }
}


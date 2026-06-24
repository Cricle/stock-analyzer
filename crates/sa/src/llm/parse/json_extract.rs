fn strip_code_fence(content: &str) -> Option<&str> {
    let fenced = content.strip_prefix("```")?;
    let body = match fenced.find('\n') {
        Some(index) => &fenced[index + 1..],
        None => return None,
    };
    let end = body.rfind("```")?;
    Some(body[..end].trim())
}

fn slice_outer_json_object(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    (start < end).then_some(content[start..=end].trim())
}

fn slice_first_complete_json_value(content: &str) -> Option<&str> {
    let bytes = content.as_bytes();
    let start = bytes.iter().position(|byte| {
        matches!(
            *byte,
            b'{' | b'[' | b'"' | b'-' | b'0'..=b'9' | b't' | b'f' | b'n'
        )
    })?;

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().enumerate().skip(start) {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match *byte {
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(content[start..=index].trim());
                }
            }
            _ => {
                if depth == 0 && index > start && byte.is_ascii_whitespace() {
                    return Some(content[start..index].trim());
                }
            }
        }
    }

    if depth == 0 && start < content.len() {
        return Some(content[start..].trim());
    }
    None
}

/// Fix LLM outputs where `[` is used instead of `{` for an object value.
/// This happens when the LLM confuses array/object brackets, e.g.:
///   "missing_evidence_ladder": ["key": "value", ...]
/// should be:
///   "missing_evidence_ladder": {"key": "value", ...}
fn repair_bracket_confusion(content: &str) -> Option<String> {
    let bytes = content.as_bytes();
    let mut result = content.to_string();
    let mut changed = false;
    let mut i = 0;

    while i < bytes.len() {
        // Look for pattern: ":  [" followed (after whitespace) by a quoted key and colon
        if bytes[i] == b':' {
            let ws_start = i + 1;
            let mut j = ws_start;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'[' {
                // Check if the next non-whitespace content is a "key": pattern
                let mut k = j + 1;
                while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                    k += 1;
                }
                if k < bytes.len() && bytes[k] == b'"' {
                    // Find end of this string
                    let mut m = k + 1;
                    while m < bytes.len() {
                        if bytes[m] == b'\\' {
                            m += 2;
                            continue;
                        }
                        if bytes[m] == b'"' {
                            break;
                        }
                        m += 1;
                    }
                    if m < bytes.len() {
                        let mut n = m + 1;
                        while n < bytes.len() && bytes[n].is_ascii_whitespace() {
                            n += 1;
                        }
                        if n < bytes.len() && bytes[n] == b':' {
                            // Confirmed: "[ followed by "key": — this is bracket confusion.
                            // Find the matching closing bracket.
                            let mut depth = 1i32;
                            let mut p = j + 1;
                            let mut in_str = false;
                            let mut esc = false;
                            while p < bytes.len() && depth > 0 {
                                if in_str {
                                    if esc {
                                        esc = false;
                                    } else if bytes[p] == b'\\' {
                                        esc = true;
                                    } else if bytes[p] == b'"' {
                                        in_str = false;
                                    }
                                } else {
                                    match bytes[p] {
                                        b'"' => in_str = true,
                                        b'[' => depth += 1,
                                        b']' => depth -= 1,
                                        b'{' => depth += 1,
                                        b'}' => depth -= 1,
                                        _ => {}
                                    }
                                }
                                p += 1;
                            }
                            if depth == 0 && p > 0 {
                                // Replace the opening [ with { and closing ] with }
                                let close_idx = p - 1;
                                // Safety: only replace if the close bracket is actually ]
                                if close_idx < result.len() && result.as_bytes()[close_idx] == b']' {
                                    result.replace_range(close_idx..close_idx + 1, "}");
                                    // j might have shifted if earlier replacements changed length,
                                    // but since we're doing end-first, the [ position is stable.
                                    result.replace_range(j..j + 1, "{");
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    changed.then_some(result)
}

fn repair_common_malformed_json_variants(content: &str) -> Option<String> {
    let mut repaired = content.to_string();
    let mut changed = false;

    // Fix bracket confusion first (e.g. "[" used instead of "{" for objects)
    if let Some(fixed) = repair_bracket_confusion(&repaired) {
        repaired = fixed;
        changed = true;
    }

    for field in [
        "response",
        "detail",
        "summary",
        "reasoning",
        "rationale",
        "strategic_actions",
        "investment_plan",
        "trader_plan",
        "executive_summary",
        "investment_thesis",
        "risk_assessment",
        "reflection",
    ] {
        while let Some(next) = collapse_adjacent_string_literals_for_field(&repaired, field) {
            repaired = next;
            changed = true;
        }
    }

    while let Some(next) = wrap_misquoted_nested_key_value(&repaired) {
        repaired = next;
        changed = true;
    }

    changed.then_some(repaired)
}

fn wrap_misquoted_nested_key_value(content: &str) -> Option<String> {
    let bytes = content.as_bytes();
    let mut index = 0usize;
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
            b'"' => {
                in_string = true;
                index += 1;
            }
            b':' => {
                let nested_key_start = skip_json_whitespace(content, index + 1);
                if bytes.get(nested_key_start).copied() != Some(b'"') {
                    index += 1;
                    continue;
                }

                let nested_key_end = find_json_string_end(content, nested_key_start)?;
                let nested_colon = skip_json_whitespace(content, nested_key_end + 1);
                if bytes.get(nested_colon).copied() != Some(b':') {
                    index += 1;
                    continue;
                }

                let value_start = skip_json_whitespace(content, nested_colon + 1);
                let value_end = find_json_value_end(content, value_start)?;
                let replacement = format!("{{{}}}", &content[nested_key_start..=value_end]);
                return Some(format!(
                    "{}{}{}",
                    &content[..nested_key_start],
                    replacement,
                    &content[value_end + 1..]
                ));
            }
            _ => index += 1,
        }
    }

    None
}

fn collapse_adjacent_string_literals_for_field(content: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let start = content.find(&key)?;
    let after_key = start + key.len();
    let colon_offset = content[after_key..].find(':')?;
    let value_start = skip_json_whitespace(content, after_key + colon_offset + 1);
    if content.as_bytes().get(value_start).copied()? != b'"' {
        return None;
    }

    let first_end = find_json_string_end(content, value_start)?;
    let mut fragments = vec![decode_json_string_literal(&content[value_start..=first_end]).ok()?];
    let mut cursor = first_end + 1;
    let bytes = content.as_bytes();
    let mut changed = false;

    loop {
        let comma_index = skip_json_whitespace(content, cursor);
        if bytes.get(comma_index).copied() != Some(b',') {
            break;
        }
        let next_start = skip_json_whitespace(content, comma_index + 1);
        if bytes.get(next_start).copied() != Some(b'"') {
            break;
        }
        let next_end = find_json_string_end(content, next_start)?;
        let after_string = skip_json_whitespace(content, next_end + 1);
        if bytes.get(after_string).copied() == Some(b':') {
            break;
        }
        fragments.push(decode_json_string_literal(&content[next_start..=next_end]).ok()?);
        cursor = next_end + 1;
        changed = true;
    }

    if !changed {
        return None;
    }

    let merged = serde_json::to_string(&fragments.join("\n\n")).ok()?;
    Some(format!(
        "{}{}{}",
        &content[..value_start],
        merged,
        &content[cursor..]
    ))
}

#[cfg(test)]
mod json_extract_tests {
    use super::*;

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
        assert_eq!(slice_first_complete_json_value(input), Some("[{\"a\":1},{\"b\":2}]"));
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
}


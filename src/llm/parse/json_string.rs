/// Advance past ASCII whitespace starting at the given index.
pub fn skip_json_whitespace(content: &str, mut index: usize) -> usize {
    let bytes = content.as_bytes();
    while let Some(byte) = bytes.get(index) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        index += 1;
    }
    index
}

/// Find the closing `"` of a JSON string starting at the opening `"`, respecting escapes.
pub fn find_json_string_end(content: &str, start: usize) -> Option<usize> {
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

/// Find the end index of a JSON value (string, object, array, or primitive) starting at `start`.
pub fn find_json_value_end(content: &str, start: usize) -> Option<usize> {
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

/// Decode a JSON string literal (with escape sequences) into a plain string.
pub fn decode_json_string_literal(literal: &str) -> anyhow::Result<String> {
    serde_json::from_str::<String>(literal).map_err(Into::into)
}

/// Extract a simple string field value from raw JSON text by key name.
pub fn extract_simple_json_string_field(content: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let start = content.find(&key)?;
    let after_key = start + key.len();
    let colon = content[after_key..].find(':')? + after_key;
    let value_start = skip_json_whitespace(content, colon + 1);
    let value_end = find_json_string_end(content, value_start)?;
    decode_json_string_literal(&content[value_start..=value_end]).ok()
}

/// Extract a string field value from malformed JSON, bounded by the next known field key.
pub fn extract_relaxed_json_string_field(
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

/// Extract a JSON value that appears before a known next-field key in raw text.
pub fn extract_json_value_before_known_field(
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

/// Normalize a relaxed JSON string by unescaping common escape sequences.
pub fn normalize_relaxed_json_string(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('"').trim_end();
    trimmed
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\r", "\r")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}



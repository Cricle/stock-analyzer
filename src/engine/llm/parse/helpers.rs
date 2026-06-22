pub fn text_or_default(value: Option<Value>, default: &str) -> String {
    value
        .map(|value| normalize_value(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Like [`text_or_default`], but also returns the i18n key when the fallback
/// is used.  The second element is `Some(key)` when the LLM did not provide a
/// value and the default placeholder was substituted.
pub fn text_or_default_with_key(
    value: Option<Value>,
    _default: &str,
    key: &str,
) -> (String, Option<String>) {
    let text = value
        .map(|value| normalize_value(&value))
        .filter(|value| !value.is_empty());
    match text {
        Some(t) => (t, None),
        None => (String::new(), Some(key.to_string())),
    }
}

pub fn first_non_empty(values: &[Option<&Value>], default: &str) -> String {
    values
        .iter()
        .filter_map(|value| *value)
        .map(normalize_value)
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Like [`first_non_empty`], but also returns the i18n key when the fallback
/// is used.
pub fn first_non_empty_with_key(
    values: &[Option<&Value>],
    _default: &str,
    key: &str,
) -> (String, Option<String>) {
    let text = values
        .iter()
        .filter_map(|value| *value)
        .map(normalize_value)
        .find(|value| !value.is_empty());
    match text {
        Some(t) => (t, None),
        None => (String::new(), Some(key.to_string())),
    }
}

pub fn string_list_or_default(value: Option<Value>, _defaults: &[&str]) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => {
            items
                .iter()
                .map(normalize_inline_value)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        }
        Some(other) => {
            let text = normalize_value(&other);
            if text.is_empty() {
                Vec::new()
            } else {
                text.lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(|line| line.to_string())
                    .collect()
            }
        }
        None => Vec::new(),
    }
}

/// Like [`string_list_or_default`], but also returns the i18n key when the
/// fallback is used.  The second element is `Some(key)` when the LLM did not
/// provide a value and the default placeholders were substituted.
pub fn string_list_or_default_with_key(
    value: Option<Value>,
    _defaults: &[&str],
    key: &str,
) -> (Vec<String>, Option<String>) {
    let result = string_list_or_default(value, &[]);
    if result.is_empty() {
        (result, Some(key.to_string()))
    } else {
        (result, None)
    }
}

pub fn normalize_probability(value: &Value) -> f64 {
    match value {
        Value::Number(number) => number.as_f64().map(clamp_probability).unwrap_or(0.0),
        Value::String(text) => {
            let trimmed = text.trim().trim_end_matches('%');
            trimmed
                .parse::<f64>()
                .map(|value| {
                    if text.chars().any(|ch| ch == '%') || value > 1.0 {
                        clamp_probability(value / 100.0)
                    } else {
                        clamp_probability(value)
                    }
                })
                .unwrap_or(0.0)
        }
        Value::Array(items) => items
            .iter()
            .find_map(|item| {
                let normalized = normalize_probability(item);
                (normalized > 0.0).then_some(normalized)
            })
            .unwrap_or(0.0),
        Value::Object(map) => {
            for key in [
                "value",
                "probability",
                "score",
                "confidence",
                "up",
                "down",
                "sideways",
            ] {
                if let Some(value) = map.get(key) {
                    let normalized = normalize_probability(value);
                    if normalized > 0.0 {
                        return normalized;
                    }
                }
            }
            map.values()
                .find_map(|item| {
                    let normalized = normalize_probability(item);
                    (normalized > 0.0).then_some(normalized)
                })
                .unwrap_or(0.0)
        }
        Value::Bool(_) | Value::Null => 0.0,
    }
}

pub(crate) fn normalize_numeric(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => parse_first_numeric_token(text),
        Value::Array(items) => items.iter().find_map(normalize_numeric),
        Value::Object(map) => {
            for key in ["value", "amount", "price", "target", "entry", "stop"] {
                if let Some(value) = map.get(key).and_then(normalize_numeric) {
                    return Some(value);
                }
            }
            map.values().find_map(normalize_numeric)
        }
        Value::Bool(_) | Value::Null => None,
    }
}

pub fn normalize_probability_triplet(
    up: &Value,
    down: &Value,
    sideways: &Value,
) -> (f64, f64, f64) {
    let up = normalize_probability(up);
    let down = normalize_probability(down);
    let sideways = normalize_probability(sideways);
    let total = up + down + sideways;

    if total <= f64::EPSILON {
        return (0.33, 0.33, 0.34);
    }

    let up = up / total;
    let down = down / total;
    let sideways = sideways / total;
    let drift = 1.0 - (up + down + sideways);

    (up, down, (sideways + drift).clamp(0.0, 1.0))
}

fn clamp_probability(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn parse_first_numeric_token(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = trimmed.parse::<f64>() {
        return Some(value);
    }

    let chars: Vec<char> = trimmed.chars().collect();
    let mut idx = 0;
    while idx < chars.len() {
        // Skip to the start of a numeric token
        if !(chars[idx].is_ascii_digit() || matches!(chars[idx], '+' | '-' | '.')) {
            idx += 1;
            continue;
        }

        let mut token = String::new();
        token.push(chars[idx]);
        idx += 1;

        // Collect the rest of the numeric token
        while idx < chars.len() && (chars[idx].is_ascii_digit() || chars[idx] == '.') {
            token.push(chars[idx]);
            idx += 1;
        }

        if token.is_empty() || token == "+" || token == "-" || token == "." {
            continue;
        }

        // Skip numbers followed by period/MA indicator characters
        // (e.g. "200日均线" where 200 is a period, not a price)
        if idx < chars.len() && matches!(chars[idx], '日' | '天' | '周' | '月' | '年' | '均' | '线' | 'd' | 'D' | 'w' | 'W' | 'm' | 'M' | 'y' | 'Y') {
            continue;
        }

        if let Ok(value) = token.parse::<f64>()
            && value.is_finite() && value > 0.0 {
                return Some(value);
            }
    }

    None
}

pub fn normalize_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Array(items) => items
            .iter()
            .map(normalize_inline_value)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| format!("{key}: {}", normalize_inline_value(value)))
            .filter(|line| !line.ends_with(": "))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn normalize_inline_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Array(items) => items
            .iter()
            .map(normalize_inline_value)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("; "),
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| format!("{key}: {}", normalize_inline_value(value)))
            .filter(|segment| !segment.ends_with(": "))
            .collect::<Vec<_>>()
            .join("; "),
    }
}

pub(crate) fn is_default_text(value: &str) -> bool {
    value.trim().is_empty()
}

#[cfg(test)]
mod helpers_tests {
    use super::*;
    use serde_json::json;

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
        assert_eq!(string_list_or_default(Some(val), &["x"]), vec!["a", "b", "c"]);
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
        assert_eq!(string_list_or_default(Some(val), &["x"]), vec!["a", "b", "c"]);
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
        let (up, down, sideways) =
            normalize_probability_triplet(&json!(0.5), &json!(0.3), &json!(0.2));
        assert!((up - 0.5).abs() < 0.01);
        assert!((down - 0.3).abs() < 0.01);
        assert!((sideways - 0.2).abs() < 0.01);
    }

    #[test]
    fn normalize_probability_triplet_unbalanced() {
        let (up, down, sideways) =
            normalize_probability_triplet(&json!(50), &json!(30), &json!(20));
        let total = up + down + sideways;
        assert!((total - 1.0).abs() < 0.01);
    }

    #[test]
    fn normalize_probability_triplet_all_zero() {
        let (up, down, sideways) =
            normalize_probability_triplet(&json!(0), &json!(0), &json!(0));
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
}


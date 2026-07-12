use serde_json::Value;

/// Normalize a JSON value to a probability in [0.0, 1.0].
///
/// Handles numbers, percentage strings ("75%"), arrays (first non-zero),
/// and objects (looks for keys like "probability", "confidence", etc.).
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

/// Convert any JSON value to a human-readable string.
///
/// Objects are rendered as `key: value` lines; arrays as newline-joined items.
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
    normalize_inline_value_with_sep(value, ", ")
}

/// Convert a JSON value to a compact inline string with a custom separator.
pub fn normalize_inline_value_with_sep(value: &Value, sep: &str) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Array(items) => items
            .iter()
            .map(|v| normalize_inline_value_with_sep(v, sep))
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(sep),
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| format!("{key}: {}", normalize_inline_value_with_sep(value, sep)))
            .filter(|segment| !segment.ends_with(": "))
            .collect::<Vec<_>>()
            .join(sep),
    }
}

/// Clamp a value to the [0.0, 1.0] probability range.
pub fn clamp_probability(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

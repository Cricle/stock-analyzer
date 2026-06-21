use serde_json::Value;

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
            .join(", "),
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| format!("{key}: {}", normalize_inline_value(value)))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn clamp_probability(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod value_utils_tests {
    use super::*;
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
}

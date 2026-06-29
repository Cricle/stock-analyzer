pub fn text_or_default(value: Option<Value>, default: &str) -> String {
    value
        .map(|value| normalize_value(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub fn first_non_empty(values: &[Option<&Value>], default: &str) -> String {
    values
        .iter()
        .filter_map(|value| *value)
        .map(normalize_value)
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub fn string_list_or_default(value: Option<Value>, defaults: &[&str]) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => {
            let values = items
                .iter()
                .map(normalize_inline_value)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>();
            if values.is_empty() {
                defaults.iter().map(|item| item.to_string()).collect()
            } else {
                values
            }
        }
        Some(other) => {
            let text = normalize_value(&other);
            if text.is_empty() {
                defaults.iter().map(|item| item.to_string()).collect()
            } else {
                text.lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(|line| line.to_string())
                    .collect()
            }
        }
        None => defaults.iter().map(|item| item.to_string()).collect(),
    }
}

pub use crate::value_utils::normalize_probability;

fn normalize_inline_value(value: &Value) -> String {
    crate::value_utils::normalize_inline_value_with_sep(value, "; ")
}

pub fn normalize_numeric(value: &Value) -> Option<f64> {
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

pub fn parse_first_numeric_token(text: &str) -> Option<f64> {
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
        if idx < chars.len() && matches!(chars[idx], '日' | '天' | '周' | '月' | '年' | '均' | '线') {
            continue;
        }

        if let Ok(value) = token.parse::<f64>()
            && value.is_finite() && value > 0.0 {
                return Some(value);
            }
    }

    None
}

pub use crate::value_utils::normalize_value;

pub fn is_default_text(value: &str) -> bool {
    matches!(
        value,
        "模型未返回该角色摘要。"
            | "模型未返回该角色详细分析。"
            | "模型未返回该角色依据。"
            | "模型未返回分析师动作原因。"
            | "模型未返回辩论内容。"
            | "模型未返回研究经理结论。"
            | "模型未返回交易员计划。"
            | "模型未返回组合经理决策。"
    )
}



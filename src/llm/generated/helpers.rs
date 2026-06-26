use regex::Regex;
use serde_json::Value;

use super::super::parse;
use super::types::GeneratedMissingEvidenceLadder;

/// Helper for extracting fields from a JSON object during `from_value` parsing.
pub(crate) struct FieldExtractor<'a> {
    object: Option<&'a serde_json::Map<String, Value>>,
}

impl<'a> FieldExtractor<'a> {
    pub fn from_raw(raw: &'a Value) -> Self {
        Self { object: raw.as_object() }
    }

    pub fn field(&self, key: &str) -> Option<Value> {
        self.object.and_then(|map| map.get(key)).cloned()
    }

    pub fn text(&self, key: &str, default: &str) -> String {
        parse::text_or_default(self.field(key), default)
    }
}

pub fn role_report_probabilities(
    up: Option<Value>,
    down: Option<Value>,
    sideways: Option<Value>,
    _summary: &str,
    _detail: &str,
    _rationale: &str,
    _next_steps: &[String],
) -> (Value, Value, Value) {
    let valid_up = up.filter(|v| is_meaningful_value(v) && !is_zero_value(v));
    let valid_down = down.filter(|v| is_meaningful_value(v) && !is_zero_value(v));
    let valid_sideways = sideways.filter(|v| is_meaningful_value(v) && !is_zero_value(v));

    if let (Some(up_value), Some(down_value), Some(sideways_value)) =
        (valid_up.clone(), valid_down.clone(), valid_sideways.clone())
    {
        if !is_uniform_distribution(&up_value, &down_value, &sideways_value) {
            return (up_value, down_value, sideways_value);
        }
    }

    // No text-based fallback — return LLM values or neutral defaults.
    (
        valid_up.unwrap_or(Value::from(0.33)),
        valid_down.unwrap_or(Value::from(0.33)),
        valid_sideways.unwrap_or(Value::from(0.34)),
    )
}

/// Detect uniform ~33/33/33 distribution from LLM — means no real conviction.
pub fn is_uniform_distribution(up: &Value, down: &Value, sideways: &Value) -> bool {
    let to_f64 = |v: &Value| -> Option<f64> {
        match v {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    };
    let (Some(u), Some(d), Some(s)) = (to_f64(up), to_f64(down), to_f64(sideways)) else {
        return false;
    };
    (u - 0.333).abs() < 0.02 && (d - 0.333).abs() < 0.02 && (s - 0.333).abs() < 0.02
}

pub fn is_zero_value(value: &Value) -> bool {
    match value {
        Value::Number(n) => n.as_f64().map_or(false, |f| f.abs() < f64::EPSILON),
        Value::String(s) => s.trim() == "0" || s.trim() == "0.0",
        _ => false,
    }
}

pub fn meaningful_value(value: Option<Value>) -> Option<Value> {
    value.filter(is_meaningful_value)
}

pub fn object_value(value: Option<Value>) -> Option<Value> {
    meaningful_value(value)
}

pub fn derive_probabilities_from_text(
    summary: &str,
    detail: &str,
    rationale: &str,
) -> (f64, f64, f64) {
    let combined = format!("{summary} {detail} {rationale}").to_lowercase();

    let bullish_keywords = [
        "bullish", "upside", "growth", "positive", "strong", "buy", "accumulate", "breakout",
        "momentum", "outperform", "upgrade", "catalyst", "recovery", "反弹", "修复", "企稳",
        "回升", "站稳", "看多", "看涨", "买入", "增持", "突破", "上涨", "利好", "强势",
        "增长", "积极", "乐观", "超预期", "放量", "拉升", "支撑", "金叉", "背离",
    ];
    let bearish_keywords = [
        "bearish", "downside", "risk", "negative", "weak", "sell", "reduce", "breakdown",
        "decline", "underperform", "downgrade", "headwind", "偏空", "承压", "派发", "下行",
        "超卖", "死叉", "看空", "看跌", "卖出", "减持", "跌破", "下跌", "利空", "弱势",
        "下滑", "消极", "悲观", "低于预期", "缩量", "回调", "失守", "加速下跌", "卖压",
        "抛售", "止损",
    ];

    let bull_count = bullish_keywords
        .iter()
        .filter(|k| combined.contains(*k))
        .count();
    let bear_count = bearish_keywords
        .iter()
        .filter(|k| combined.contains(*k))
        .count();
    let total = bull_count + bear_count;

    if total == 0 {
        return (0.25, 0.40, 0.35);
    }

    let bull_ratio = bull_count as f64 / total as f64;
    let bear_ratio = bear_count as f64 / total as f64;

    let up = (bull_ratio * 0.6 + 0.1).min(0.80);
    let down = (bear_ratio * 0.6 + 0.1).min(0.80);
    let sideways = (1.0 - up - down).max(0.10);

    let sum = up + down + sideways;
    (up / sum, down / sum, sideways / sum)
}

pub fn extract_numbered_trigger_lines(text: &str) -> Vec<String> {
    let pattern = Regex::new(r"(?m)(?:^|[;；。:：]\s*)(?:\d+[\)\.]|[1-9]）)\s*([^\n;；。]+)").ok();
    pattern
        .map(|regex| {
            regex
                .captures_iter(text)
                .filter_map(|caps| caps.get(1).map(|m| m.as_str().trim().to_string()))
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn format_price_like_text(value: f64) -> String {
    if (value - value.round()).abs() < 0.01 {
        format!("{:.0}", value)
    } else if value.abs() >= 1000.0 {
        format!("{:.1}", value)
    } else {
        format!("{:.2}", value)
    }
}

pub fn extract_time_horizon_from_texts(texts: &[&str]) -> Option<String> {
    let regex = Regex::new(r"(?i)\b(\d+\s*-\s*\d+\s*周|\d+\s*周|\d+\s*-\s*\d+\s*weeks?)\b").ok()?;
    texts.iter().find_map(|text| {
        regex
            .captures(text)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().trim().to_string()))
    })
}

pub fn extract_position_sizing_from_texts(texts: &[&str]) -> Option<String> {
    let regex = Regex::new(r"(?i)\b(\d{1,2}(?:\.\d+)?\s*%)").ok()?;
    texts.iter().find_map(|text| {
        regex
            .captures(text)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().trim().to_string()))
    })
}

pub fn is_meaningful_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Number(_) | Value::Bool(_) => true,
        Value::Array(items) => items.iter().any(is_meaningful_value),
        Value::Object(map) => map.values().any(is_meaningful_value),
    }
}

pub fn extract_object_value(value: Option<&Value>, keys: &[&str]) -> Option<Value> {
    let Value::Object(map) = value? else {
        return None;
    };

    keys.iter()
        .find_map(|key| map.get(*key))
        .cloned()
        .filter(is_meaningful_value)
}

pub fn extract_object_string_list(value: Option<&Value>, keys: &[&str]) -> Vec<String> {
    for key in keys {
        let items = extract_object_value(value, &[*key])
            .map(|candidate| split_list_like_value(&candidate))
            .unwrap_or_default();
        if !items.is_empty() {
            return items;
        }
    }
    Vec::new()
}

fn split_list_like_value(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => dedup_string_items(items.iter().flat_map(split_list_like_value)),
        Value::Object(map) => dedup_string_items(map.values().flat_map(split_list_like_value)),
        _ => split_list_like_text(&parse::normalize_value(value)),
    }
}

pub fn split_list_like_text(text: &str) -> Vec<String> {
    dedup_string_items(
        text.lines()
            .flat_map(|line| line.split([';', '；']))
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned),
    )
}

fn dedup_string_items<I>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut unique = Vec::new();
    for item in items {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if unique.iter().any(|existing| existing == trimmed) {
            continue;
        }
        unique.push(trimmed.to_string());
    }
    unique
}

impl GeneratedMissingEvidenceLadder {
    pub(crate) fn from_value(raw: Option<Value>) -> Self {
        let object = raw.as_ref().and_then(Value::as_object);
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let nested = meaningful_value(field("missing_evidence_ladder"))
            .or_else(|| meaningful_value(field("missing_evidence")))
            .or_else(|| meaningful_value(field("missing_evidence_classification")))
            .or_else(|| meaningful_value(field("missing_evidence_severity_ladder")));
        if nested.is_some() {
            return Self::from_value(nested);
        }
        Self {
            tolerable_gaps: {
                let direct = meaningful_value(field("tolerable_gaps"))
                    .or_else(|| meaningful_value(field("tolerable_context_gaps")));
                direct
                    .as_ref()
                    .map(split_list_like_value)
                    .unwrap_or_default()
            },
            manageable_gaps: {
                let direct = meaningful_value(field("manageable_gaps"))
                    .or_else(|| meaningful_value(field("serious_but_manageable_gaps")));
                direct
                    .as_ref()
                    .map(split_list_like_value)
                    .unwrap_or_default()
            },
            blocking_gaps: {
                let direct = meaningful_value(field("blocking_gaps"))
                    .or_else(|| meaningful_value(field("decision_blocking_gaps")));
                direct
                    .as_ref()
                    .map(split_list_like_value)
                    .unwrap_or_default()
            },
        }
    }
}

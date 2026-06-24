use regex::Regex;
use serde_json::Value;

use super::super::parse;
use super::types::GeneratedMissingEvidenceLadder;

pub(super) fn role_report_probabilities(
    up: Option<Value>,
    down: Option<Value>,
    sideways: Option<Value>,
    summary: &str,
    detail: &str,
    rationale: &str,
    _next_steps: &[String],
) -> (Value, Value, Value) {
    let valid_up = up.filter(|v| is_meaningful_value(v) && !is_zero_value(v));
    let valid_down = down.filter(|v| is_meaningful_value(v) && !is_zero_value(v));
    let valid_sideways = sideways.filter(|v| is_meaningful_value(v) && !is_zero_value(v));

    if let (Some(up_value), Some(down_value), Some(sideways_value)) =
        (valid_up.clone(), valid_down.clone(), valid_sideways.clone())
    {
        // Check if the values form a uniform distribution (33/33/33) — this means
        // the LLM didn't actually form a view. Fall back to text derivation.
        if !is_uniform_distribution(&up_value, &down_value, &sideways_value) {
            return (up_value, down_value, sideways_value);
        }
    }

    let (derived_up, derived_down, derived_sideways) =
        derive_probabilities_from_text(summary, detail, rationale);

    (
        valid_up.unwrap_or(Value::from(derived_up)),
        valid_down.unwrap_or(Value::from(derived_down)),
        valid_sideways.unwrap_or(Value::from(derived_sideways)),
    )
}

/// Detect uniform ~33/33/33 distribution from LLM — means no real conviction.
fn is_uniform_distribution(up: &Value, down: &Value, sideways: &Value) -> bool {
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
    // Check if all three are within 0.01 of 1/3
    (u - 0.333).abs() < 0.02 && (d - 0.333).abs() < 0.02 && (s - 0.333).abs() < 0.02
}

fn is_zero_value(value: &Value) -> bool {
    match value {
        Value::Number(n) => n.as_f64().map_or(false, |f| f.abs() < f64::EPSILON),
        Value::String(s) => s.trim() == "0" || s.trim() == "0.0",
        _ => false,
    }
}

fn derive_probabilities_from_text(summary: &str, detail: &str, rationale: &str) -> (f64, f64, f64) {
    let combined = format!("{summary} {detail} {rationale}").to_lowercase();

    let bullish_keywords = [
        "bullish",
        "upside",
        "growth",
        "positive",
        "strong",
        "buy",
        "accumulate",
        "breakout",
        "momentum",
        "outperform",
        "upgrade",
        "catalyst",
        "recovery",
        "反弹",
        "修复",
        "企稳",
        "回升",
        "站稳",
        "看多",
        "看涨",
        "买入",
        "增持",
        "突破",
        "上涨",
        "利好",
        "强势",
        "增长",
        "积极",
        "乐观",
        "超预期",
        "放量",
        "拉升",
        "支撑",
        "金叉",
        "背离",
    ];
    let bearish_keywords = [
        "bearish",
        "downside",
        "risk",
        "negative",
        "weak",
        "sell",
        "reduce",
        "breakdown",
        "decline",
        "underperform",
        "downgrade",
        "headwind",
        "偏空",
        "承压",
        "派发",
        "下行",
        "超卖",
        "死叉",
        "看空",
        "看跌",
        "卖出",
        "减持",
        "跌破",
        "下跌",
        "利空",
        "弱势",
        "下滑",
        "消极",
        "悲观",
        "低于预期",
        "缩量",
        "回调",
        "失守",
        "加速下跌",
        "卖压",
        "抛售",
        "止损",
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
        // Default to slightly bearish when no signals detected (conservative)
        return (0.25, 0.40, 0.35);
    }

    let bull_ratio = bull_count as f64 / total as f64;
    let bear_ratio = bear_count as f64 / total as f64;

    let up = (0.10 + bull_ratio * 0.65).clamp(0.08, 0.75);
    let down = (0.10 + bear_ratio * 0.65).clamp(0.08, 0.75);
    let sideways = (1.0 - up - down).clamp(0.08, 0.55);

    (up, down, sideways)
}

pub(super) fn extract_numbered_trigger_lines(text: &str) -> Vec<String> {
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

pub(super) fn meaningful_value(value: Option<Value>) -> Option<Value> {
    value.filter(is_meaningful_value)
}

pub(super) fn object_value(value: Option<Value>) -> Option<Value> {
    meaningful_value(value)
}

fn is_meaningful_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Number(_) | Value::Bool(_) => true,
        Value::Array(items) => items.iter().any(is_meaningful_value),
        Value::Object(map) => map.values().any(is_meaningful_value),
    }
}

pub(super) fn extract_object_value(value: Option<&Value>, keys: &[&str]) -> Option<Value> {
    let Value::Object(map) = value? else {
        return None;
    };

    keys.iter()
        .find_map(|key| map.get(*key))
        .cloned()
        .filter(is_meaningful_value)
}

pub(super) fn extract_object_string_list(value: Option<&Value>, keys: &[&str]) -> Vec<String> {
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

fn split_list_like_text(text: &str) -> Vec<String> {
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

pub(super) fn extract_entry_price_from_texts(texts: &[&str]) -> Option<f64> {
    extract_first_price_after_keywords(texts, &["回踩", "承接", "支撑", "站稳"])
}

pub(super) fn extract_stop_loss_from_texts(texts: &[&str]) -> Option<f64> {
    extract_first_price_after_keywords(
        texts,
        &["跌破", "失守", "止损", "invalidation", "invalidations"],
    )
}

pub(super) fn extract_price_target_from_texts(texts: &[&str]) -> Option<f64> {
    extract_first_price_after_keywords(texts, &["前高", "突破", "目标", "price target"])
}

pub(super) fn extract_time_horizon_from_texts(texts: &[&str]) -> Option<String> {
    let regex = Regex::new(r"(?i)\b(\d+\s*-\s*\d+\s*周|\d+\s*周|\d+\s*-\s*\d+\s*weeks?)\b").ok()?;
    texts.iter().find_map(|text| {
        regex
            .captures(text)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().trim().to_string()))
    })
}

pub(super) fn extract_position_sizing_from_texts(texts: &[&str]) -> Option<String> {
    let regex = Regex::new(r"(?i)\b(\d{1,2}(?:\.\d+)?\s*%)").ok()?;
    texts.iter().find_map(|text| {
        regex
            .captures(text)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().trim().to_string()))
    })
}

pub(super) fn format_price_like_text(value: f64) -> String {
    if (value - value.round()).abs() < 0.01 {
        format!("{:.0}", value)
    } else if value.abs() >= 1000.0 {
        format!("{:.1}", value)
    } else {
        format!("{:.2}", value)
    }
}

fn extract_first_price_after_keywords(texts: &[&str], keywords: &[&str]) -> Option<f64> {
    let regex = Regex::new(r"(-?\d{2,5}(?:\.\d{1,4})?)").ok()?;
    texts.iter().find_map(|text| {
        let lowercase = text.to_lowercase();
        for keyword in keywords {
            let keyword_lower = keyword.to_lowercase();
            if let Some(index) = lowercase.find(&keyword_lower) {
                let tail = &text[index..];
                for caps in regex.captures_iter(tail) {
                    if let Some(m) = caps.get(1) {
                        let match_end = m.end();
                        // Skip numbers followed by period/MA indicator characters
                        // (e.g. "200日均线" where 200 is a period, not a price)
                        let after = &tail[match_end..];
                        if after.starts_with(['日', '天', '周', '月', '年', '均', '线']) {
                            continue;
                        }
                        if let Ok(value) = m.as_str().parse::<f64>() {
                            return Some(value);
                        }
                    }
                }
            }
        }
        None
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_zero_value_number() {
        assert!(is_zero_value(&json!(0)));
        assert!(is_zero_value(&json!(0.0)));
        assert!(!is_zero_value(&json!(1)));
    }

    #[test]
    fn test_is_zero_value_string() {
        assert!(is_zero_value(&json!("0")));
        assert!(is_zero_value(&json!("0.0")));
        assert!(is_zero_value(&json!(" 0 ")));
        assert!(!is_zero_value(&json!("1")));
    }

    #[test]
    fn test_is_zero_value_other() {
        assert!(!is_zero_value(&json!(true)));
        assert!(!is_zero_value(&json!(null)));
        assert!(!is_zero_value(&json!([0])));
    }

    #[test]
    fn test_is_meaningful_value_null() {
        assert!(!is_meaningful_value(&json!(null)));
    }

    #[test]
    fn test_is_meaningful_value_empty_string() {
        assert!(!is_meaningful_value(&json!("")));
        assert!(!is_meaningful_value(&json!("  ")));
    }

    #[test]
    fn test_is_meaningful_value_number() {
        assert!(is_meaningful_value(&json!(42)));
        assert!(is_meaningful_value(&json!(0)));
    }

    #[test]
    fn test_is_meaningful_value_bool() {
        assert!(is_meaningful_value(&json!(true)));
        assert!(is_meaningful_value(&json!(false)));
    }

    #[test]
    fn test_is_meaningful_value_array() {
        assert!(is_meaningful_value(&json!(["a"])));
        assert!(!is_meaningful_value(&json!([])));
        assert!(!is_meaningful_value(&json!([null, ""])));
    }

    #[test]
    fn test_is_meaningful_value_object() {
        assert!(is_meaningful_value(&json!({"key": "value"})));
        assert!(!is_meaningful_value(&json!({"key": null})));
    }

    #[test]
    fn test_is_uniform_distribution() {
        assert!(is_uniform_distribution(
            &json!(0.333),
            &json!(0.333),
            &json!(0.334)
        ));
        assert!(!is_uniform_distribution(
            &json!(0.5),
            &json!(0.3),
            &json!(0.2)
        ));
    }

    #[test]
    fn test_is_uniform_distribution_string_values() {
        assert!(is_uniform_distribution(
            &json!("0.333"),
            &json!("0.333"),
            &json!("0.333")
        ));
    }

    #[test]
    fn test_derive_probabilities_no_keywords() {
        let (up, down, sideways) = derive_probabilities_from_text("neutral text", "", "");
        assert!((up - 0.25).abs() < 0.01);
        assert!((down - 0.40).abs() < 0.01);
        assert!((sideways - 0.35).abs() < 0.01);
    }

    #[test]
    fn test_derive_probabilities_bullish() {
        let (up, down, _sideways) =
            derive_probabilities_from_text("bullish growth breakout momentum", "", "");
        assert!(up > down);
    }

    #[test]
    fn test_derive_probabilities_bearish() {
        let (up, down, _sideways) =
            derive_probabilities_from_text("bearish decline breakdown risk", "", "");
        assert!(down > up);
    }

    #[test]
    fn test_derive_probabilities_chinese_keywords() {
        let (up, down, _) = derive_probabilities_from_text("看多 突破 上涨 利好", "", "");
        assert!(up > down);
    }

    #[test]
    fn test_extract_numbered_trigger_lines() {
        let text = "1) Buy at support\n2) Sell at resistance\n3) Hold position";
        let lines = extract_numbered_trigger_lines(text);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "Buy at support");
    }

    #[test]
    fn test_extract_numbered_trigger_lines_empty() {
        assert!(extract_numbered_trigger_lines("").is_empty());
        assert!(extract_numbered_trigger_lines("no numbers here").is_empty());
    }

    #[test]
    fn test_meaningful_value_some() {
        assert!(meaningful_value(Some(json!("hello"))).is_some());
        assert!(meaningful_value(Some(json!(42))).is_some());
        assert!(meaningful_value(Some(json!(null))).is_none());
        assert!(meaningful_value(None).is_none());
    }

    #[test]
    fn test_extract_object_value() {
        let obj = json!({"key1": "value1", "key2": 42});
        assert_eq!(
            extract_object_value(Some(&obj), &["key1"]),
            Some(json!("value1"))
        );
        assert_eq!(extract_object_value(Some(&obj), &["missing"]), None);
        assert_eq!(extract_object_value(None, &["key1"]), None);
    }

    #[test]
    fn test_extract_object_string_list() {
        let obj = json!({"items": "a;b;c"});
        let result = extract_object_string_list(Some(&obj), &["items"]);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_split_list_like_text() {
        let result = split_list_like_text("a;b;c");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_split_list_like_text_newlines() {
        let result = split_list_like_text("line1\nline2\nline3");
        assert_eq!(result, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_split_list_like_text_dedup() {
        let result = split_list_like_text("a;b;a;c");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_dedup_string_items() {
        let items = vec!["a".to_string(), "b".to_string(), "a".to_string()];
        let result = dedup_string_items(items);
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    fn test_dedup_string_items_trim() {
        let items = vec![" a ".to_string(), "a".to_string()];
        let result = dedup_string_items(items);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_format_price_like_text() {
        assert_eq!(format_price_like_text(100.0), "100");
        assert_eq!(format_price_like_text(100.5), "100.50");
        assert_eq!(format_price_like_text(1234.5), "1234.5");
    }

    #[test]
    fn test_extract_entry_price_from_texts() {
        let texts = vec!["回踩 25.5 支撑"];
        assert_eq!(extract_entry_price_from_texts(&texts), Some(25.5));
    }

    #[test]
    fn test_extract_entry_price_no_match() {
        let texts = vec!["no keywords here"];
        assert_eq!(extract_entry_price_from_texts(&texts), None);
    }

    #[test]
    fn test_extract_stop_loss_from_texts() {
        let texts = vec!["跌破 20.0 止损"];
        assert_eq!(extract_stop_loss_from_texts(&texts), Some(20.0));
    }

    #[test]
    fn test_extract_price_target_from_texts() {
        let texts = vec!["目标 30.0"];
        assert_eq!(extract_price_target_from_texts(&texts), Some(30.0));
    }

    #[test]
    fn test_extract_time_horizon() {
        let texts = vec!["预计 4-6 周"];
        let result = extract_time_horizon_from_texts(&texts);
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_time_horizon_english() {
        let texts = vec!["target 2-3 weeks"];
        let result = extract_time_horizon_from_texts(&texts);
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_position_sizing() {
        let texts = vec!["仓位 30%"];
        let result = extract_position_sizing_from_texts(&texts);
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_first_price_skips_period_numbers() {
        let texts = vec!["站稳 200日均线上方"];
        let result = extract_first_price_after_keywords(&texts, &["站稳"]);
        assert!(result.is_none());
    }

    #[test]
    fn test_role_report_probabilities_valid_values() {
        let (u, d, s) = role_report_probabilities(
            Some(json!(0.5)),
            Some(json!(0.3)),
            Some(json!(0.2)),
            "",
            "",
            "",
            &[],
        );
        assert_eq!(u, json!(0.5));
        assert_eq!(d, json!(0.3));
        assert_eq!(s, json!(0.2));
    }

    #[test]
    fn test_role_report_probabilities_uniform_fallback() {
        let (u, d, _s) = role_report_probabilities(
            Some(json!(0.333)),
            Some(json!(0.333)),
            Some(json!(0.334)),
            "bullish growth",
            "",
            "",
            &[],
        );
        let u_f64 = u.as_f64().unwrap();
        let d_f64 = d.as_f64().unwrap();
        assert!(u_f64 > d_f64);
    }

    #[test]
    fn test_role_report_probabilities_none_values() {
        let (u, d, s) = role_report_probabilities(None, None, None, "neutral", "", "", &[]);
        assert!(u.is_number());
        assert!(d.is_number());
        assert!(s.is_number());
    }

    #[test]
    fn test_role_report_probabilities_zero_filtered() {
        let (u, _d, _s) = role_report_probabilities(
            Some(json!(0)),
            Some(json!(0)),
            Some(json!(0)),
            "bullish growth positive",
            "",
            "",
            &[],
        );
        let u_f64 = u.as_f64().unwrap();
        assert!(u_f64 > 0.1);
    }
}

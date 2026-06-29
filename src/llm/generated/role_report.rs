use serde_json::Value;

use super::super::parse;
use super::helpers::{meaningful_value, role_report_probabilities};
use super::types::{
    GeneratedAnalystDecision, GeneratedRoleReport, GeneratedSubscriptionQaAnswer,
    GeneratedSubscriptionQaSnapshot, ToolCall,
};

impl GeneratedRoleReport {
    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let summary = parse::text_or_default(field("summary"), "模型未返回该角色摘要。");
        let detail = parse::text_or_default(field("detail"), "模型未返回该角色详细分析。");
        let rationale = parse::text_or_default(field("rationale"), "模型未返回该角色依据。");
        let next_steps = parse::string_list_or_default(field("next_steps"), &["继续跟踪后续数据"]);
        let (up_probability, down_probability, sideways_probability) = role_report_probabilities(
            field("up_probability"),
            field("down_probability"),
            field("sideways_probability"),
            &summary,
            &detail,
            &rationale,
            &next_steps,
        );
        Self {
            key: parse::text_or_default(field("key"), "overview"),
            title: parse::text_or_default(field("title"), "总览"),
            agent: parse::text_or_default(field("agent"), "综合分析 Agent"),
            summary,
            detail,
            evidence_points: parse::string_list_or_default(
                field("evidence_points"),
                &["缺少结构化证据条目"],
            ),
            up_probability,
            down_probability,
            sideways_probability,
            confidence: field("confidence").unwrap_or(Value::String("unknown".to_string())),
            rationale,
            next_steps,
            risks: parse::string_list_or_default(field("risks"), &["需关注信息缺口与市场波动"]),
        }
    }
}

impl GeneratedAnalystDecision {
    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let tool_calls = field("tool_calls")
            .and_then(|v| {
                if let Value::Array(arr) = v {
                    Some(
                        arr.into_iter()
                            .filter_map(|item| {
                                let obj = item.as_object()?;
                                Some(ToolCall {
                                    tool_name: obj.get("tool_name")?.as_str()?.to_string(),
                                    tool_arguments: obj
                                        .get("tool_arguments")
                                        .cloned()
                                        .unwrap_or(Value::Object(Default::default())),
                                })
                            })
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();
        Self {
            action: parse::text_or_default(field("action"), "finalize"),
            reasoning: parse::text_or_default(field("reasoning"), "模型未返回分析师动作原因。"),
            final_report: field("final_report").map(GeneratedRoleReport::from_value),
            tool_name: field("tool_name")
                .map(|value| parse::normalize_value(&value))
                .filter(|value| !value.is_empty()),
            tool_arguments: field("tool_arguments"),
            tool_calls,
        }
    }
}

impl GeneratedSubscriptionQaAnswer {
    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        Self {
            summary: parse::text_or_default(field("summary"), ""),
            conclusion: parse::text_or_default(field("conclusion"), ""),
            confidence: parse::text_or_default(field("confidence"), ""),
            evidence_points: parse::string_list_or_default(field("evidence_points"), &[]),
            key_numbers: normalize_subscription_key_numbers(parse::string_list_or_default(
                field("key_numbers"),
                &[],
            )),
            risks: parse::string_list_or_default(field("risks"), &[]),
            actions: parse::string_list_or_default(field("actions"), &[]),
            references: parse::string_list_or_default(field("references"), &[]),
            context_snapshot: meaningful_value(field("context_snapshot"))
                .map(GeneratedSubscriptionQaSnapshot::from_value)
                .unwrap_or_default(),
        }
    }
}

impl GeneratedSubscriptionQaSnapshot {
    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        Self {
            question_summary: parse::text_or_default(field("question_summary"), ""),
            conclusion: parse::text_or_default(field("conclusion"), ""),
            evidence_points: parse::string_list_or_default(field("evidence_points"), &[]),
            key_numbers: normalize_subscription_key_numbers(parse::string_list_or_default(
                field("key_numbers"),
                &[],
            )),
            open_risks: parse::string_list_or_default(field("open_risks"), &[]),
        }
    }
}

fn normalize_subscription_key_numbers(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                return String::new();
            }
            if trimmed.contains(':') || trimmed.contains('=') {
                return trimmed.to_string();
            }
            format!("metric: {trimmed}")
        })
        .filter(|item| !item.is_empty())
        .collect()
}

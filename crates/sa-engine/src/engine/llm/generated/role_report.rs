use serde_json::Value;

use super::super::parse;
use super::helpers::role_report_probabilities;
use super::types::{GeneratedAnalystDecision, GeneratedRoleReport};

impl GeneratedRoleReport {
    pub fn confidence_string(&self) -> String {
        parse::normalize_value(&self.confidence)
    }

    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let (summary, summary_key) =
            parse::text_or_default_with_key(field("summary"), "模型未返回该角色摘要。", "llm.fallback.no_summary");
        let (detail, detail_key) =
            parse::text_or_default_with_key(field("detail"), "模型未返回该角色详细分析。", "llm.fallback.no_detail");
        let (rationale, rationale_key) =
            parse::text_or_default_with_key(field("rationale"), "模型未返回该角色依据。", "llm.fallback.no_rationale");
        let (next_steps, next_steps_key) =
            parse::string_list_or_default_with_key(field("next_steps"), &["继续跟踪后续数据"], "llm.fallback.no_next_steps");
        let (up_probability, down_probability, sideways_probability) = role_report_probabilities(
            field("up_probability"),
            field("down_probability"),
            field("sideways_probability"),
            &summary,
            &detail,
            &rationale,
            &next_steps,
        );
        let (evidence_points, evidence_points_key) = parse::string_list_or_default_with_key(
            field("evidence_points"),
            &["缺少结构化证据条目"],
            "llm.fallback.no_evidence",
        );
        let (risks, risks_key) = parse::string_list_or_default_with_key(
            field("risks"),
            &["需关注信息缺口与市场波动"],
            "llm.fallback.no_risk_alt",
        );
        Self {
            key: parse::text_or_default(field("key"), "overview"),
            title: parse::text_or_default(field("title"), "总览"),
            agent: parse::text_or_default(field("agent"), "综合分析 Agent"),
            summary,
            summary_key,
            detail,
            detail_key,
            evidence_points,
            evidence_points_key,
            up_probability,
            down_probability,
            sideways_probability,
            confidence: field("confidence").unwrap_or(Value::String("unknown".to_string())),
            rationale,
            rationale_key,
            next_steps,
            next_steps_key,
            risks,
            risks_key,
        }
    }
}

impl GeneratedAnalystDecision {
    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let (reasoning, reasoning_key) = parse::text_or_default_with_key(
            field("reasoning"),
            "模型未返回分析师动作原因。",
            "llm.fallback.no_reasoning",
        );
        Self {
            action: parse::text_or_default(field("action"), "finalize"),
            reasoning,
            reasoning_key,
            final_report: field("final_report").map(GeneratedRoleReport::from_value),
            tool_name: field("tool_name")
                .map(|value| parse::normalize_value(&value))
                .filter(|value| !value.is_empty()),
            tool_arguments: field("tool_arguments"),
        }
    }
}

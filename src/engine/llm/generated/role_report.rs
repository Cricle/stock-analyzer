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
            parse::text_or_default_with_key(field("summary"), "", "llm.fallback.no_summary");
        let (detail, detail_key) =
            parse::text_or_default_with_key(field("detail"), "", "llm.fallback.no_detail");
        let (rationale, rationale_key) =
            parse::text_or_default_with_key(field("rationale"), "", "llm.fallback.no_rationale");
        let (next_steps, next_steps_key) =
            parse::string_list_or_default_with_key(field("next_steps"), &["Continue tracking follow-up data"], "llm.fallback.no_next_steps");
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
            &["No structured evidence items"],
            "llm.fallback.no_evidence",
        );
        let (risks, risks_key) = parse::string_list_or_default_with_key(
            field("risks"),
            &["Monitor information gaps and market volatility"],
            "llm.fallback.no_risk_alt",
        );
        Self {
            key: parse::text_or_default(field("key"), "overview"),
            title: parse::text_or_default(field("title"), "Overview"),
            agent: parse::text_or_default(field("agent"), "Composite Analysis Agent"),
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
            "",
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

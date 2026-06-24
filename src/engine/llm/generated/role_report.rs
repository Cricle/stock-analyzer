use serde_json::Value;

use super::super::parse;
use super::generated_impls::{meaningful_value, role_report_probabilities};
use super::generated_types::{
    GeneratedAnalystDecision, GeneratedRoleReport, GeneratedSubscriptionQaAnswer,
    GeneratedSubscriptionQaSnapshot,
};

impl GeneratedRoleReport {
    pub fn confidence_string(&self) -> String {
        parse::normalize_value(&self.confidence)
    }

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
        Self {
            action: parse::text_or_default(field("action"), "finalize"),
            reasoning: parse::text_or_default(field("reasoning"), "模型未返回分析师动作原因。"),
            final_report: field("final_report").map(GeneratedRoleReport::from_value),
            tool_name: field("tool_name")
                .map(|value| parse::normalize_value(&value))
                .filter(|value| !value.is_empty()),
            tool_arguments: field("tool_arguments"),
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- GeneratedRoleReport ---

    #[test]
    fn role_report_from_value() {
        let value = serde_json::json!({
            "key": "market",
            "title": "Market Analysis",
            "agent": "Market Analyst",
            "summary": "bullish outlook",
            "detail": "detailed analysis",
            "evidence_points": ["point1"],
            "up_probability": 0.6,
            "down_probability": 0.2,
            "sideways_probability": 0.2,
            "confidence": 75,
            "rationale": "strong trend",
            "next_steps": ["monitor"],
            "risks": ["volatility"]
        });
        let report = GeneratedRoleReport::from_value(value);
        assert_eq!(report.key, "market");
        assert_eq!(report.title, "Market Analysis");
        assert!((report.up_probability - 0.6).abs() < 0.01);
    }

    #[test]
    fn role_report_from_empty() {
        let value = serde_json::json!({});
        let report = GeneratedRoleReport::from_value(value);
        assert_eq!(report.key, "overview");
        assert_eq!(report.agent, "综合分析 Agent");
    }

    #[test]
    fn role_report_confidence_string() {
        let value = serde_json::json!({
            "confidence": "high"
        });
        let report = GeneratedRoleReport::from_value(value);
        assert_eq!(report.confidence_string(), "high");
    }

    // --- GeneratedAnalystDecision ---

    #[test]
    fn analyst_decision_use_tool() {
        let value = serde_json::json!({
            "action": "use_tool",
            "reasoning": "need data",
            "tool_name": "get_fundamentals",
            "tool_arguments": {"symbol": "AAPL"}
        });
        let decision = GeneratedAnalystDecision::from_value(value);
        assert_eq!(decision.action, "use_tool");
        assert_eq!(decision.tool_name, Some("get_fundamentals".into()));
    }

    #[test]
    fn analyst_decision_finalize() {
        let value = serde_json::json!({
            "action": "finalize",
            "reasoning": "done",
            "final_report": {
                "summary": "bullish"
            }
        });
        let decision = GeneratedAnalystDecision::from_value(value);
        assert_eq!(decision.action, "finalize");
        assert!(decision.final_report.is_some());
    }

    #[test]
    fn analyst_decision_from_empty() {
        let value = serde_json::json!({});
        let decision = GeneratedAnalystDecision::from_value(value);
        assert_eq!(decision.action, "finalize");
    }

    // --- GeneratedSubscriptionQaAnswer ---

    #[test]
    fn qa_answer_from_value() {
        let value = serde_json::json!({
            "answer": "yes",
            "confidence": 80
        });
        let answer = GeneratedSubscriptionQaAnswer::from_value(value);
        assert_eq!(answer.answer, "yes");
    }

    // --- normalize_subscription_key_numbers ---

    #[test]
    fn normalize_key_numbers_with_colon() {
        let items = vec!["P/E: 15".into()];
        let result = normalize_subscription_key_numbers(items);
        assert_eq!(result, vec!["P/E: 15"]);
    }

    #[test]
    fn normalize_key_numbers_plain() {
        let items = vec!["revenue".into()];
        let result = normalize_subscription_key_numbers(items);
        assert_eq!(result, vec!["metric: revenue"]);
    }

    #[test]
    fn normalize_key_numbers_empty() {
        let items = vec!["".into()];
        let result = normalize_subscription_key_numbers(items);
        assert!(result.is_empty());
    }
}

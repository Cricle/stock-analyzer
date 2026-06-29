use serde_json::Value;

use super::super::parse;
use super::helpers::{extract_object_string_list, extract_object_value, meaningful_value};
use super::types::{
    GeneratedDebateTurn, GeneratedMissingEvidenceLadder, GeneratedResearchManager, HasConfidence,
};

impl GeneratedDebateTurn {
    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        Self {
            speaker: parse::text_or_default(field("speaker"), "Unknown"),
            stance: parse::text_or_default(field("stance"), "neutral"),
            response: parse::text_or_default(field("response"), "模型未返回辩论内容。"),
            confidence: field("confidence").unwrap_or(Value::String("unknown".to_string())),
            evidence_points: parse::string_list_or_default(
                field("evidence_points"),
                &["缺少结构化证据条目"],
            ),
            risks: parse::string_list_or_default(field("risks"), &["需关注核心假设失效"]),
        }
    }
}

impl GeneratedResearchManager {
    pub fn rendered_plan(&self) -> String {
        [
            "# Research Plan".to_string(),
            String::new(),
            "## Decision".to_string(),
            format!("**Recommendation**: {}", self.recommendation),
            format!("**Confidence**: {}", self.confidence_string()),
            format!("**Risk Assessment**: {}", self.risk_assessment),
            String::new(),
            "## Debate Synthesis".to_string(),
            format!("**Rationale**: {}", self.rationale),
            String::new(),
            "## Trader Handoff".to_string(),
            format!("**Strategic Actions**: {}", self.strategic_actions),
        ]
        .join("\n")
    }

    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let risk_assessment_raw = meaningful_value(field("risk_assessment"));
        let strategic_actions_raw =
            meaningful_value(field("strategic_actions").or_else(|| field("investment_plan")));
        let recommendation = parse::first_non_empty(
            &[field("recommendation").as_ref(), field("rating").as_ref()],
            "Unknown",
        );
        let rationale = parse::first_non_empty(
            &[
                field("rationale").as_ref(),
                field("summary").as_ref(),
                field("investment_plan").as_ref(),
            ],
            "模型未返回研究经理依据。",
        );
        let strategic_actions = parse::text_or_default(
            strategic_actions_raw.clone(),
            "模型未返回研究经理行动方案。",
        );
        let trigger_checklist = parse::string_list_or_default(field("trigger_checklist"), &[]);
        let trigger_checklist = if trigger_checklist.is_empty() {
            extract_object_string_list(
                strategic_actions_raw.as_ref(),
                &[
                    "trigger_checklist",
                    "trigger_checklist_for_upgrading_from_hold",
                    "trigger_checklist_for_upgrade_from_hold_or_cautious_to_action",
                    "upgrade_trigger_checklist",
                ],
            )
        } else {
            trigger_checklist
        };
        Self {
            recommendation,
            confidence: field("confidence").unwrap_or(Value::String("unknown".to_string())),
            risk_assessment: parse::text_or_default(
                risk_assessment_raw.clone(),
                "模型未返回风险评估。",
            ),
            rationale,
            strategic_actions,
            missing_evidence_ladder: GeneratedMissingEvidenceLadder::from_risk_assessment(
                &field,
                risk_assessment_raw.as_ref(),
            ),
            trigger_checklist,
            accounting_scope_hypothesis: meaningful_value(field("accounting_scope_hypothesis"))
                .or_else(|| {
                    extract_object_value(
                        risk_assessment_raw.as_ref(),
                        &["accounting_scope_hypothesis", "data_scope_hypothesis"],
                    )
                })
                .map(|value| parse::normalize_value(&value))
                .filter(|value| !value.is_empty()),
        }
    }
}

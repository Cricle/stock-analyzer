use serde_json::Value;

use super::super::parse;
use super::helpers::{
    extract_object_string_list, extract_object_value, meaningful_value,
};
use super::types::{GeneratedDebateTurn, GeneratedMissingEvidenceLadder, GeneratedResearchManager};

impl GeneratedDebateTurn {
    pub fn confidence_string(&self) -> String {
        parse::normalize_value(&self.confidence)
    }

    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let (response, response_key) = parse::text_or_default_with_key(
            field("response"),
            "",
            "llm.fallback.no_debate",
        );
        let (evidence_points, evidence_points_key) = parse::string_list_or_default_with_key(
            field("evidence_points"),
            &["No structured evidence items"],
            "llm.fallback.no_evidence",
        );
        let (risks, risks_key) = parse::string_list_or_default_with_key(
            field("risks"),
            &["Monitor core assumption invalidation"],
            "llm.fallback.no_risk",
        );
        Self {
            speaker: parse::text_or_default(field("speaker"), "Unknown"),
            stance: parse::text_or_default(field("stance"), "neutral"),
            response,
            response_key,
            confidence: field("confidence").unwrap_or(Value::String("unknown".to_string())),
            evidence_points,
            evidence_points_key,
            risks,
            risks_key,
        }
    }
}

impl GeneratedResearchManager {
    pub fn confidence_string(&self) -> String {
        parse::normalize_value(&self.confidence)
    }

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
            "Hold",
        );
        let (rationale, rationale_key) = parse::first_non_empty_with_key(
            &[
                field("rationale").as_ref(),
                field("summary").as_ref(),
                field("investment_plan").as_ref(),
            ],
            "",
            "llm.fallback.no_research_rationale",
        );
        let (strategic_actions, strategic_actions_key) = parse::text_or_default_with_key(
            strategic_actions_raw.clone(),
            "",
            "llm.fallback.no_research_action",
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
        let (risk_assessment, risk_assessment_key) = parse::text_or_default_with_key(
            risk_assessment_raw.clone(),
            "",
            "llm.fallback.no_risk_assessment",
        );
        Self {
            recommendation,
            confidence: field("confidence").unwrap_or(Value::String("unknown".to_string())),
            risk_assessment,
            risk_assessment_key,
            rationale,
            rationale_key,
            strategic_actions,
            strategic_actions_key,
            missing_evidence_ladder: GeneratedMissingEvidenceLadder::from_value(
                meaningful_value(field("missing_evidence_ladder")).or_else(|| {
                    extract_object_value(
                        risk_assessment_raw.as_ref(),
                        &[
                            "missing_evidence_ladder",
                            "missing_evidence",
                            "missing_evidence_classification",
                            "missing_evidence_severity_ladder",
                        ],
                    )
                }),
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

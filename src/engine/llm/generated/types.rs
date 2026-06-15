use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedRoleReport {
    pub key: String,
    pub title: String,
    pub agent: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_key: Option<String>,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_key: Option<String>,
    pub evidence_points: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_points_key: Option<String>,
    pub up_probability: Value,
    pub down_probability: Value,
    pub sideways_probability: Value,
    pub confidence: Value,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale_key: Option<String>,
    pub next_steps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_steps_key: Option<String>,
    pub risks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risks_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedAnalystDecision {
    pub action: String,
    pub reasoning: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_key: Option<String>,
    pub final_report: Option<GeneratedRoleReport>,
    pub tool_name: Option<String>,
    pub tool_arguments: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedDebateTurn {
    pub speaker: String,
    pub stance: String,
    pub response: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_key: Option<String>,
    pub confidence: Value,
    pub evidence_points: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_points_key: Option<String>,
    pub risks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risks_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedResearchManager {
    pub recommendation: String,
    pub confidence: Value,
    pub risk_assessment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_assessment_key: Option<String>,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale_key: Option<String>,
    pub strategic_actions: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategic_actions_key: Option<String>,
    #[serde(default)]
    pub missing_evidence_ladder: GeneratedMissingEvidenceLadder,
    #[serde(default)]
    pub trigger_checklist: Vec<String>,
    #[serde(default)]
    pub accounting_scope_hypothesis: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedTraderDecision {
    pub action: String,
    pub reasoning: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_key: Option<String>,
    pub trader_plan: String,
    pub entry_price: Option<Value>,
    pub stop_loss: Option<Value>,
    #[serde(default)]
    pub confirmation_level: Option<Value>,
    #[serde(default)]
    pub target_reference: Option<String>,
    #[serde(default)]
    pub target_condition: Option<String>,
    #[serde(default)]
    pub time_horizon: Option<String>,
    pub position_sizing: Option<String>,
    #[serde(default)]
    pub execution_trigger_checklist: Vec<String>,
    #[serde(default)]
    pub blocking_gaps: Vec<String>,
    #[serde(default)]
    pub time_stop_deadline: Option<String>,
    #[serde(default)]
    pub time_stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedPortfolioDecision {
    pub rating: String,
    pub confidence: Value,
    pub risk_assessment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_assessment_key: Option<String>,
    pub summary: String,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale_key: Option<String>,
    pub executive_summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executive_summary_key: Option<String>,
    pub investment_thesis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub investment_thesis_key: Option<String>,
    pub price_target: Option<Value>,
    #[serde(default)]
    pub confirmation_level: Option<Value>,
    #[serde(default)]
    pub invalidation_level: Option<Value>,
    #[serde(default)]
    pub target_reference: Option<String>,
    #[serde(default)]
    pub target_condition: Option<String>,
    pub time_horizon: Option<String>,
    #[serde(default)]
    pub missing_evidence_ladder: GeneratedMissingEvidenceLadder,
    #[serde(default)]
    pub trigger_checklist: Vec<String>,
    #[serde(default)]
    pub scenario_paths: Vec<GeneratedScenarioPath>,
    #[serde(default)]
    pub time_stop_deadline: Option<String>,
    #[serde(default)]
    pub time_stop_reason: Option<String>,
    #[serde(default)]
    pub reflection: Option<GeneratedReflection>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedScenarioPath {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub risk_boundary: String,
    #[serde(default)]
    pub position_sizing: String,
    #[serde(default)]
    pub stop_level: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedReflection {
    #[serde(default)]
    pub strongest_part: String,
    #[serde(default)]
    pub weakest_uncertainty_or_missing_evidence: String,
    #[serde(default)]
    pub next_lesson_for_next_run: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedMissingEvidenceLadder {
    #[serde(default)]
    pub tolerable_gaps: Vec<String>,
    #[serde(default)]
    pub manageable_gaps: Vec<String>,
    #[serde(default)]
    pub blocking_gaps: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedSubscriptionQaAnswer {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub conclusion: String,
    #[serde(default)]
    pub confidence: String,
    #[serde(default)]
    pub evidence_points: Vec<String>,
    #[serde(default)]
    pub key_numbers: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub context_snapshot: GeneratedSubscriptionQaSnapshot,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedSubscriptionQaSnapshot {
    #[serde(default)]
    pub question_summary: String,
    #[serde(default)]
    pub conclusion: String,
    #[serde(default)]
    pub evidence_points: Vec<String>,
    #[serde(default)]
    pub key_numbers: Vec<String>,
    #[serde(default)]
    pub open_risks: Vec<String>,
}

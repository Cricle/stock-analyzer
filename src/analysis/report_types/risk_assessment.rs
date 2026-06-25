
use crate::types::{PendingToolCall, ToolObservation};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StructuredReflection {
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub strengths: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub uncertainties: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub next_lessons: LocalText,
    #[serde(default)]
    pub raw_reflection: String,
    #[serde(default, skip_serializing)]
    pub markdown: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StructuredRiskAssessment {
    #[serde(default)]
    pub decision_blocking_gaps: Vec<String>,
    #[serde(default)]
    pub key_risks: Vec<String>,
    #[serde(default)]
    pub offsetting_supports: Vec<String>,
    #[serde(default)]
    pub invalidation_conditions: Vec<String>,
    #[serde(default)]
    pub overall_risk_framing: String,
    #[serde(default)]
    pub serious_but_manageable_gaps: Vec<String>,
    #[serde(default)]
    pub tolerable_context_gaps: Vec<String>,
    #[serde(default)]
    pub raw_text: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportStageState {
    pub overview: bool,
    pub market: bool,
    pub fundamentals: bool,
    pub news: bool,
    pub sentiment: bool,
    pub bull_research: bool,
    pub bear_research: bool,
    pub research_plan: bool,
    pub trader_plan: bool,
    pub risk_debate: bool,
    pub portfolio_decision: bool,
    pub reflection: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalystRuntimeState {
    pub key: String,
    #[serde(default)]
    pub pending_tool: Option<PendingToolCall>,
    #[serde(default)]
    pub tool_history: Vec<ToolObservation>,
    #[serde(default)]
    pub final_messages: Vec<String>,
    #[serde(default)]
    pub cleared: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeNodeTrace {
    pub stage: String,
    pub node: String,
    pub step: i64,
    pub timestamp: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentReportNode {
    pub key: String,
    pub title: String,
    pub agent: String,
    pub summary: String,
    pub detail: String,
    #[serde(default)]
    pub evidence_points: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_probability_value")]
    pub up_probability: f64,
    #[serde(default, deserialize_with = "deserialize_probability_value")]
    pub down_probability: f64,
    #[serde(default, deserialize_with = "deserialize_probability_value")]
    pub sideways_probability: f64,
    #[serde(default, deserialize_with = "deserialize_string_value")]
    pub confidence: String,
    pub rationale: String,
    #[serde(default)]
    pub next_steps: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
}

fn deserialize_probability_value<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(crate::value_utils::normalize_probability(&value))
}

fn deserialize_string_value<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(crate::value_utils::normalize_value(&value))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DebateTurn {
    pub speaker: String,
    pub stance: String,
    pub response: String,
    pub confidence: String,
    #[serde(default)]
    pub evidence_points: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InvestmentDebateState {
    pub bull_history: String,
    pub bear_history: String,
    pub history: String,
    pub current_response: String,
    pub judge_decision: String,
    pub count: i32,
    #[serde(default)]
    pub turns: Vec<DebateTurn>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RiskDebateState {
    pub aggressive_history: String,
    pub conservative_history: String,
    pub neutral_history: String,
    pub history: String,
    pub latest_speaker: String,
    pub current_aggressive_response: String,
    pub current_conservative_response: String,
    pub current_neutral_response: String,
    pub judge_decision: String,
    pub count: i32,
    #[serde(default)]
    pub turns: Vec<DebateTurn>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReflectionState {
    pub status: String,
    pub reflection: String,
    pub source: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalysisCheckpoint {
    pub stage_key: String,
    pub stage_name: String,
    pub status: String,
    pub summary: String,
    pub generated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisTaskSummary {
    pub task_id: String,
    pub stock_code: String,
    pub stock_name: String,
    pub market_type: String,
    pub status: TaskStatus,
    pub progress: i32,
    pub start_time: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub llm_token_usage: LlmTokenUsageSummary,
}

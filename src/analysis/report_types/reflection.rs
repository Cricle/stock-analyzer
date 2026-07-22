
/// A scenario path within an action guide (trigger, action, risk boundary).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActionScenarioPath {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: LocalText,
    #[serde(default)]
    pub trigger: LocalText,
    #[serde(default)]
    pub action: LocalText,
    #[serde(default)]
    pub risk_boundary: LocalText,
    /// Per-path position sizing guidance, e.g. "计划仓位的50%（不超过总资金5%）"
    #[serde(default)]
    pub position_sizing: LocalText,
    /// Per-path specific stop-loss or invalidation price level
    #[serde(default)]
    pub stop_level: LocalText,
    /// When true, `position_sizing` is intentionally empty because IC
    /// discipline forbids new positions (e.g. "no_attack" state).  The
    /// frontend should render this as "observation only" without inventing
    /// a sizing number.
    #[serde(default)]
    pub sizing_blocked: bool,
}

/// Structured research plan with recommendation, confidence, and risk assessment.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StructuredResearchPlan {
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub recommendation: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub confidence: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub risk_assessment: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub rationale: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub strategic_actions: LocalText,
    #[serde(default)]
    pub missing_evidence_ladder: MissingEvidenceLadder,
    #[serde(default)]
    pub trigger_checklist: Vec<String>,
    #[serde(default)]
    pub accounting_scope_hypothesis: String,
    #[serde(default, skip_serializing)]
    pub markdown: String,
}

/// Structured trader plan with entry, stop, target, and position sizing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StructuredTraderPlan {
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub action: LocalText,
    #[serde(default)]
    pub raw_action: String,
    #[serde(default)]
    pub calibrated_action: String,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub reasoning: LocalText,
    pub entry_price: String,
    pub stop_loss: String,
    #[serde(default)]
    pub confirmation_level: String,
    #[serde(default)]
    pub target_reference: String,
    #[serde(default)]
    pub target_condition: String,
    #[serde(default)]
    pub time_horizon: String,
    pub position_sizing: String,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub proposal: LocalText,
    #[serde(default)]
    pub execution_trigger_checklist: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_execution_discipline: Option<StopExecutionDiscipline>,
    #[serde(default)]
    pub blocking_gaps: Vec<String>,
    /// Time-based stop-loss deadline, e.g. "业绩说明会后10个交易日"
    #[serde(default)]
    pub time_stop_deadline: String,
    /// Reason for the time stop, e.g. "催化剂落空后回归纯现金观察"
    #[serde(default)]
    pub time_stop_reason: String,
    #[serde(default, skip_serializing)]
    pub markdown: String,
}

/// Code-generated stop policy. Presentation is owned by the frontend so the
/// backend exposes only direction and numeric thresholds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StopExecutionDiscipline {
    pub breach_direction: StopExecutionBreachDirection,
    pub stop_price: f64,
    pub immediate_exit_threshold_pct: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopExecutionBreachDirection {
    #[default]
    Below,
    Above,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Structured portfolio decision.
pub struct StructuredPortfolioDecision {
    pub rating: Rating,
    #[serde(default)]
    pub raw_rating: String,
    #[serde(default)]
    pub calibrated_rating: String,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub confidence: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub risk_assessment: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub executive_summary: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub investment_thesis: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub rationale: LocalText,
    pub price_target: String,
    #[serde(default)]
    pub confirmation_level: String,
    #[serde(default)]
    pub invalidation_level: String,
    #[serde(default)]
    pub target_type: String,
    #[serde(default)]
    pub target_reference: String,
    #[serde(default)]
    pub target_condition: String,
    pub time_horizon: String,
    #[serde(default)]
    pub missing_evidence_ladder: MissingEvidenceLadder,
    #[serde(default)]
    pub trigger_checklist: Vec<String>,
    #[serde(default, skip_serializing)]
    pub markdown: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Missing evidence ladder.
pub struct MissingEvidenceLadder {
    #[serde(default)]
    pub tolerable_gaps: Vec<String>,
    #[serde(default)]
    pub manageable_gaps: Vec<String>,
    #[serde(default)]
    pub blocking_gaps: Vec<String>,
}


/// Event-driven catalyst scoring card for evaluating earnings calls, guidance, etc.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CatalystScoreCard {
    /// Name of the catalyst event, e.g. "2025Q2业绩说明会"
    #[serde(default)]
    pub event_name: String,
    /// Scoring items: each is a (question, score: 0 or 1) pair
    #[serde(default)]
    pub items: Vec<CatalystScoreItem>,
    /// Total score across all items
    #[serde(default)]
    pub total_score: i32,
    /// Maximum possible score
    #[serde(default)]
    pub max_score: i32,
    /// Interpretation of the total score, e.g. "积极 — 上调关注度"
    #[serde(default)]
    pub interpretation: LocalText,
    /// Recommended next action based on score
    #[serde(default)]
    pub recommended_action: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Catalyst score item.
pub struct CatalystScoreItem {
    /// The evaluation question, e.g. "管理层是否明确指引毛利率改善？"
    #[serde(default)]
    pub question: LocalText,
    /// Score: 1 if yes, 0 if no
    #[serde(default)]
    pub score: i32,
    /// Optional evidence or notes
    #[serde(default)]
    pub evidence: LocalText,
}

/// Daily and weekly review checklist for ongoing monitoring
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReviewChecklist {
    /// Daily review items (check after market close)
    #[serde(default)]
    pub daily: Vec<ReviewItem>,
    /// Weekly review items (check on weekends)
    #[serde(default)]
    pub weekly: Vec<ReviewItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Review item.
pub struct ReviewItem {
    /// What to check, e.g. "价格是否接近61.75或66.0？"
    #[serde(default)]
    pub check: LocalText,
    /// Category: price, technical, fundamental, discipline
    #[serde(default)]
    pub category: String,
    /// Priority: high, medium, low
    #[serde(default)]
    pub priority: String,
}

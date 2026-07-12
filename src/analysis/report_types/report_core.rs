
/// Deserialize a `LocalText` from either a `{ "key": ..., "params": {...} }` object
/// or a plain string (for backward compatibility with pre-i18n data).
fn deserialize_local_text_or_string<'de, D>(deserializer: D) -> Result<LocalText, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => Ok(LocalText::new(s)),
        Value::Object(_) => serde_json::from_value(value)
            .map_err(serde::de::Error::custom),
        _ => Ok(LocalText::default()),
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Structured report.
pub struct StructuredReport {
    #[serde(default)]
    pub report_flavor: ReportFlavor,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub title: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub summary: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub recommendation: LocalText,
    #[serde(default)]
    pub raw_llm_recommendation: String,
    #[serde(default, skip_serializing)]
    pub recommendation_calibration_reason: String,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub confidence: LocalText,
    #[serde(default)]
    pub raw_llm_confidence: String,
    #[serde(default)]
    pub confidence_score: i32,
    #[serde(default)]
    pub confidence_breakdown: ConfidenceBreakdown,
    #[serde(default)]
    pub confidence_profile: ConfidenceProfile,
    #[serde(default)]
    pub confidence_caps: Vec<ConfidenceCap>,
    #[serde(default)]
    pub research_reliability: ResearchReliability,
    #[serde(default)]
    pub research_confidence_score: i32,
    #[serde(default)]
    pub direction_score: i32,
    #[serde(default)]
    pub direction_breakdown: DirectionBreakdown,
    #[serde(default)]
    pub action_score: i32,
    #[serde(default)]
    pub action_breakdown: ActionBreakdown,
    #[serde(default)]
    pub execution_readiness: ExecutionReadiness,
    #[serde(default)]
    pub trade_setup_quality: TradeSetupQuality,
    #[serde(default)]
    pub calibration_summary: CalibrationSummary,
    #[serde(default)]
    pub diagnostics: ReportDiagnostics,
    #[serde(default)]
    pub references: ReportReferenceSnapshot,
    #[serde(default)]
    pub market_chart: ReportMarketChart,
    #[serde(default)]
    pub user_context: AnalysisUserContext,
    #[serde(default)]
    pub price_context: PriceContext,
    #[serde(default)]
    pub probability_view: ProbabilityView,
    #[serde(default)]
    pub profit_risk: ProfitRiskView,
    #[serde(default)]
    pub ic_navigator: IcNavigatorView,
    #[serde(default)]
    pub ic_discipline: IcDisciplineView,
    #[serde(default)]
    pub technical_indicators: TechnicalIndicatorView,
    #[serde(default)]
    pub evidence_cards: Vec<ReportEvidenceCard>,
    #[serde(default)]
    pub news_insights: Vec<NewsInsight>,
    #[serde(default)]
    pub risk_controls: Vec<RiskControl>,
    #[serde(default)]
    pub action_guides: ReportActionGuides,
    #[serde(default)]
    pub decision_view: DecisionView,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub core_research_call: LocalText,
    #[serde(default)]
    pub mispricing_claim: LocalText,
    #[serde(default)]
    pub why_now: LocalText,
    #[serde(default)]
    pub required_confirmation: LocalText,
    #[serde(default)]
    pub max_initial_risk_budget: LocalText,
    #[serde(default)]
    pub appendix_reliability_summary: String,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub risk_assessment: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub rationale: LocalText,
    #[serde(default)]
    pub research_plan: StructuredResearchPlan,
    #[serde(default)]
    pub trader_plan: StructuredTraderPlan,
    #[serde(default)]
    pub portfolio_decision: StructuredPortfolioDecision,
    #[serde(default)]
    pub reflection: StructuredReflection,
    /// Event-driven catalyst scoring card (e.g. earnings call evaluation)
    #[serde(default)]
    pub catalyst_score_card: CatalystScoreCard,
    /// Daily/weekly review checklist for ongoing monitoring
    #[serde(default)]
    pub review_checklist: ReviewChecklist,
    #[serde(default)]
    pub stage_state: ReportStageState,
    #[serde(default, skip_serializing)]
    pub sections: Vec<ReportSection>,
}

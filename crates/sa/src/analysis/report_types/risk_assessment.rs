
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

#[cfg(test)]
mod risk_assessment_tests {
    use super::*;

    #[test]
    fn structured_reflection_serde_roundtrip() {
        let r = StructuredReflection {
            strengths: LocalText::new("strong_brand"),
            uncertainties: LocalText::new("market_volatility"),
            next_lessons: LocalText::new("watch_earnings"),
            raw_reflection: "raw text".into(),
            markdown: "md".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let restored: StructuredReflection = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.strengths.key, "strong_brand");
        assert_eq!(restored.raw_reflection, "raw text");
    }

    #[test]
    fn structured_reflection_from_legacy_string() {
        let json = r#"{"strengths":"good","uncertainties":"bad","next_lessons":"learn"}"#;
        let r: StructuredReflection = serde_json::from_str(json).unwrap();
        assert_eq!(r.strengths.key, "good");
    }

    #[test]
    fn structured_risk_assessment_serde_roundtrip() {
        let r = StructuredRiskAssessment {
            decision_blocking_gaps: vec!["gap1".into()],
            key_risks: vec!["risk1".into()],
            offsetting_supports: vec!["s1".into()],
            invalidation_conditions: vec!["c1".into()],
            overall_risk_framing: "moderate".into(),
            serious_but_manageable_gaps: vec!["g2".into()],
            tolerable_context_gaps: vec!["g3".into()],
            raw_text: "raw".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let restored: StructuredRiskAssessment = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.key_risks.len(), 1);
        assert_eq!(restored.overall_risk_framing, "moderate");
    }

    #[test]
    fn report_stage_state_serde_roundtrip() {
        let s = ReportStageState {
            overview: true, market: true, fundamentals: false,
            news: true, sentiment: false, bull_research: true,
            bear_research: true, research_plan: true, trader_plan: true,
            risk_debate: true, portfolio_decision: true, reflection: false,
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: ReportStageState = serde_json::from_str(&json).unwrap();
        assert!(restored.overview);
        assert!(!restored.fundamentals);
    }

    #[test]
    fn analyst_runtime_state_serde_roundtrip() {
        let s = AnalystRuntimeState {
            key: "market".into(),
            pending_tool: Some(PendingToolCall {
                tool_name: "search".into(),
                arguments: serde_json::json!({"q": "AAPL"}),
                reason: "need data".into(),
            }),
            tool_history: vec![ToolObservation {
                tool_name: "search".into(),
                arguments: serde_json::json!({}),
                output: "results".into(),
                meta: serde_json::json!({}),
                success: true,
                created_at: "2025-01-01T00:00:00Z".into(),
            }],
            final_messages: vec!["done".into()],
            cleared: false,
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: AnalystRuntimeState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.key, "market");
        assert!(restored.pending_tool.is_some());
    }

    #[test]
    fn agent_report_node_serde_roundtrip() {
        let n = AgentReportNode {
            key: "market".into(), title: "Market".into(), agent: "m".into(),
            summary: "s".into(), detail: "d".into(),
            evidence_points: vec!["e1".into()],
            up_probability: 0.6, down_probability: 0.2, sideways_probability: 0.2,
            confidence: "high".into(), rationale: "r".into(),
            next_steps: vec!["s1".into()], risks: vec!["r1".into()],
        };
        let json = serde_json::to_string(&n).unwrap();
        let restored: AgentReportNode = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.key, "market");
        assert!((restored.up_probability - 0.6).abs() < 0.01);
    }

    #[test]
    fn agent_report_node_string_probabilities() {
        let json = r#"{"key":"t","title":"","agent":"","summary":"","detail":"","up_probability":"0.5","down_probability":"0.3","sideways_probability":"0.2","confidence":"","rationale":""}"#;
        let n: AgentReportNode = serde_json::from_str(json).unwrap();
        assert!((n.up_probability - 0.5).abs() < 0.01);
    }

    #[test]
    fn debate_turn_serde_roundtrip() {
        let t = DebateTurn {
            speaker: "bull".into(), stance: "aggressive".into(),
            response: "buy".into(), confidence: "high".into(),
            evidence_points: vec!["e1".into()], risks: vec!["r1".into()],
        };
        let json = serde_json::to_string(&t).unwrap();
        let restored: DebateTurn = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.speaker, "bull");
    }

    #[test]
    fn investment_debate_state_serde_roundtrip() {
        let s = InvestmentDebateState {
            bull_history: "b".into(), bear_history: "be".into(),
            history: "h".into(), current_response: "r".into(),
            judge_decision: "j".into(), count: 3, turns: vec![],
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: InvestmentDebateState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.count, 3);
    }

    #[test]
    fn risk_debate_state_serde_roundtrip() {
        let s = RiskDebateState {
            aggressive_history: "a".into(), conservative_history: "c".into(),
            neutral_history: "n".into(), history: "h".into(),
            latest_speaker: "conservative".into(),
            current_aggressive_response: "ar".into(),
            current_conservative_response: "cr".into(),
            current_neutral_response: "nr".into(),
            judge_decision: "j".into(), count: 2, turns: vec![],
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: RiskDebateState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.count, 2);
    }

    #[test]
    fn analysis_checkpoint_serde_roundtrip() {
        let c = AnalysisCheckpoint {
            stage_key: "market".into(), stage_name: "Market".into(),
            status: "done".into(), summary: "ok".into(),
            generated_at: "2025-01-01".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let restored: AnalysisCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.stage_key, "market");
    }

    #[test]
    fn analysis_task_summary_serde_roundtrip() {
        let s = AnalysisTaskSummary {
            task_id: "t1".into(), stock_code: "AAPL".into(),
            stock_name: "Apple".into(), market_type: "美股".into(),
            status: TaskStatus::Running, progress: 50,
            start_time: "2025-01-01".into(), created_at: "2025-01-01".into(),
            updated_at: "2025-01-01".into(),
            llm_token_usage: LlmTokenUsageSummary::default(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: AnalysisTaskSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.task_id, "t1");
        assert_eq!(restored.status, TaskStatus::Running);
    }

    #[test]
    fn runtime_node_trace_serde_roundtrip() {
        let t = RuntimeNodeTrace {
            stage: "m".into(), node: "a".into(), step: 3,
            timestamp: "2025-01-01".into(),
        };
        let json = serde_json::to_string(&t).unwrap();
        let restored: RuntimeNodeTrace = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.step, 3);
    }

    #[test]
    fn reflection_state_serde_roundtrip() {
        let s = ReflectionState {
            status: "done".into(), reflection: "r".into(), source: "s".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let restored: ReflectionState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.status, "done");
    }

    #[test]
    fn all_defaults() {
        assert!(StructuredReflection::default().strengths.is_empty());
        assert!(StructuredRiskAssessment::default().key_risks.is_empty());
        assert!(!ReportStageState::default().overview);
        assert!(AnalystRuntimeState::default().key.is_empty());
        assert!(DebateTurn::default().speaker.is_empty());
        assert_eq!(InvestmentDebateState::default().count, 0);
        assert_eq!(RiskDebateState::default().count, 0);
        assert!(ReflectionState::default().status.is_empty());
        assert!(AnalysisCheckpoint::default().stage_key.is_empty());
    }
}

pub(crate) fn validate_research_manager(parsed: &super::super::GeneratedResearchManager, raw: &str) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
    if parsed.recommendation == "Hold" && !raw.contains("recommendation") && !raw.contains("rating")
    {
        issues.push(DiagnosisIssue::warning(
            "research_manager", "recommendation",
            "recommendation defaulted to Hold (field missing)",
        ));
    }
    if is_default_text(&parsed.rationale) {
        issues.push(DiagnosisIssue::error(
            "research_manager", "rationale",
            "rationale is default placeholder",
        ));
    }
    if is_default_text(&parsed.risk_assessment) {
        issues.push(DiagnosisIssue::error(
            "research_manager", "risk_assessment",
            "risk_assessment is default placeholder",
        ));
    }
    if !issues.is_empty() {
        tracing::warn!(
            issues = %issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(", "),
            raw_len = raw.len(),
            "LLM output schema validation: parsed research manager has quality issues"
        );
    }
    issues
}

pub fn validate_analyst_decision(parsed: &super::super::GeneratedAnalystDecision, raw: &str) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
    if is_default_text(&parsed.reasoning) {
        issues.push(DiagnosisIssue::error(
            "analyst_decision", "reasoning",
            "reasoning is default placeholder",
        ));
    }
    if parsed.action == "finalize" && parsed.final_report.is_none() {
        issues.push(DiagnosisIssue::error(
            "analyst_decision", "final_report",
            "finalize action but no final_report",
        ));
    }
    if parsed.action == "tool"
        && (parsed.tool_name.is_none() || parsed.tool_name.as_deref() == Some(""))
    {
        issues.push(DiagnosisIssue::error(
            "analyst_decision", "tool_name",
            "tool action but no tool_name",
        ));
    }
    if !issues.is_empty() {
        tracing::warn!(
            issues = %issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(", "),
            action = %parsed.action,
            raw_len = raw.len(),
            "LLM output schema validation: parsed analyst decision has quality issues"
        );
    }
    issues
}

pub(crate) fn validate_debate_turn(parsed: &super::super::GeneratedDebateTurn, raw: &str) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
    if is_default_text(&parsed.response) {
        issues.push(DiagnosisIssue::error(
            "debate_turn", "response",
            "response is default placeholder",
        ));
    }
    if parsed.speaker == "Unknown" {
        issues.push(DiagnosisIssue::warning(
            "debate_turn", "speaker",
            "speaker defaulted to Unknown",
        ));
    }
    if parsed.evidence_points.len() == 1 && parsed.evidence_points[0] == "缺少结构化证据条目"
    {
        issues.push(DiagnosisIssue::warning(
            "debate_turn", "evidence_points",
            "evidence_points is default placeholder",
        ));
    }
    if !issues.is_empty() {
        tracing::warn!(
            issues = %issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(", "),
            speaker = %parsed.speaker,
            raw_len = raw.len(),
            "LLM output schema validation: parsed debate turn has quality issues"
        );
    }
    issues
}

pub(crate) fn validate_trader_decision(parsed: &super::super::GeneratedTraderDecision, raw: &str) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
    if is_default_text(&parsed.trader_plan) {
        issues.push(DiagnosisIssue::error(
            "trader_decision", "trader_plan",
            "trader_plan is default placeholder",
        ));
    }
    if is_default_text(&parsed.reasoning) {
        issues.push(DiagnosisIssue::error(
            "trader_decision", "reasoning",
            "reasoning is default placeholder",
        ));
    }
    if !issues.is_empty() {
        tracing::warn!(
            issues = %issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(", "),
            action = %parsed.action,
            raw_len = raw.len(),
            "LLM output schema validation: parsed trader decision has quality issues"
        );
    }
    issues
}

pub(crate) fn validate_portfolio_decision(parsed: &super::super::GeneratedPortfolioDecision, raw: &str) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
    if is_default_text(&parsed.executive_summary) {
        issues.push(DiagnosisIssue::error(
            "portfolio_decision", "executive_summary",
            "executive_summary is default placeholder",
        ));
    }
    if is_default_text(&parsed.rationale) {
        issues.push(DiagnosisIssue::error(
            "portfolio_decision", "rationale",
            "rationale is default placeholder",
        ));
    }
    if parsed.rating == "Hold" && !raw.contains("rating") && !raw.contains("recommendation") {
        issues.push(DiagnosisIssue::warning(
            "portfolio_decision", "rating",
            "rating defaulted to Hold (field missing)",
        ));
    }
    if !issues.is_empty() {
        tracing::warn!(
            issues = %issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(", "),
            rating = %parsed.rating,
            raw_len = raw.len(),
            "LLM output schema validation: parsed portfolio decision has quality issues"
        );
    }
    issues
}

#[cfg(test)]
mod validate_tests {
    use super::super::*;

    #[test]
    fn validate_research_manager_default_rationale() {
        let parsed = super::super::super::super::GeneratedResearchManager {
            recommendation: "Buy".to_string(),
            confidence: serde_json::Value::from(80),
            rationale: "模型未返回该角色依据。".to_string(),
            risk_assessment: "some risk".to_string(),
            strategic_actions: "actions".to_string(),
            trigger_checklist: vec![],
            missing_evidence_ladder: Default::default(),
            accounting_scope_hypothesis: None,
        };
        let issues = validate_research_manager(&parsed, "raw content with recommendation");
        assert!(issues.iter().any(|i| i.field == "rationale"));
    }

    #[test]
    fn validate_analyst_decision_finalize_without_report() {
        let parsed = super::super::super::super::GeneratedAnalystDecision {
            action: "finalize".to_string(),
            reasoning: "good reasoning".to_string(),
            final_report: None,
            tool_name: None,
            tool_arguments: None,
        };
        let issues = validate_analyst_decision(&parsed, "raw");
        assert!(issues.iter().any(|i| i.field == "final_report"));
    }

    #[test]
    fn validate_analyst_decision_tool_without_name() {
        let parsed = super::super::super::super::GeneratedAnalystDecision {
            action: "tool".to_string(),
            reasoning: "need more data".to_string(),
            final_report: None,
            tool_name: Some("".to_string()),
            tool_arguments: None,
        };
        let issues = validate_analyst_decision(&parsed, "raw");
        assert!(issues.iter().any(|i| i.field == "tool_name"));
    }

    #[test]
    fn validate_debate_turn_default_response() {
        let parsed = super::super::super::super::GeneratedDebateTurn {
            speaker: "Bull".to_string(),
            stance: "bullish".to_string(),
            response: "模型未返回辩论内容。".to_string(),
            confidence: serde_json::Value::from(70),
            evidence_points: vec!["point1".to_string()],
            risks: vec!["risk1".to_string()],
        };
        let issues = validate_debate_turn(&parsed, "raw");
        assert!(issues.iter().any(|i| i.field == "response"));
    }

    #[test]
    fn validate_debate_turn_unknown_speaker() {
        let parsed = super::super::super::super::GeneratedDebateTurn {
            speaker: "Unknown".to_string(),
            stance: "neutral".to_string(),
            response: "good response".to_string(),
            confidence: serde_json::Value::from(50),
            evidence_points: vec!["point".to_string()],
            risks: vec!["risk".to_string()],
        };
        let issues = validate_debate_turn(&parsed, "raw");
        assert!(issues.iter().any(|i| i.field == "speaker"));
    }

    #[test]
    fn validate_portfolio_decision_default_executive_summary() {
        let parsed = super::super::super::super::GeneratedPortfolioDecision {
            rating: "Buy".to_string(),
            confidence: serde_json::Value::from(85),
            executive_summary: "模型未返回该角色摘要。".to_string(),
            rationale: "good rationale".to_string(),
            risk_assessment: "some risk".to_string(),
            summary: "test".to_string(),
            investment_thesis: "test".to_string(),
            price_target: None,
            confirmation_level: None,
            invalidation_level: None,
            target_reference: None,
            target_condition: None,
            time_horizon: None,
            missing_evidence_ladder: Default::default(),
            trigger_checklist: vec![],
            scenario_paths: vec![],
            time_stop_deadline: None,
            time_stop_reason: None,
            reflection: None,
        };
        let issues = validate_portfolio_decision(&parsed, "raw with rating");
        assert!(issues.iter().any(|i| i.field == "executive_summary"));
    }

    #[test]
    fn validate_trader_decision_default_plan() {
        let parsed = super::super::super::super::GeneratedTraderDecision {
            action: "Buy".to_string(),
            reasoning: "good reasoning".to_string(),
            trader_plan: "模型未返回交易员计划。".to_string(),
            entry_price: None,
            stop_loss: None,
            confirmation_level: None,
            target_reference: None,
            target_condition: None,
            time_horizon: None,
            position_sizing: None,
            execution_trigger_checklist: vec![],
            blocking_gaps: vec![],
            time_stop_deadline: None,
            time_stop_reason: None,
        };
        let issues = validate_trader_decision(&parsed, "raw");
        assert!(issues.iter().any(|i| i.field == "trader_plan"));
    }
}

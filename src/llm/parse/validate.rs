pub fn validate_research_manager(parsed: &super::GeneratedResearchManager, raw: &str) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
    if parsed.recommendation == "Unknown" {
        issues.push(DiagnosisIssue::error(
            "research_manager", "recommendation",
            "recommendation not extracted from LLM response",
        ));
    } else if parsed.recommendation == "Hold"
        && !raw.contains("recommendation")
        && !raw.contains("rating")
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

pub fn validate_analyst_decision(parsed: &super::GeneratedAnalystDecision, raw: &str) -> Vec<DiagnosisIssue> {
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

pub fn validate_debate_turn(parsed: &super::GeneratedDebateTurn, raw: &str) -> Vec<DiagnosisIssue> {
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

pub fn validate_trader_decision(parsed: &super::GeneratedTraderDecision, raw: &str) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
    if parsed.action == "Unknown" {
        issues.push(DiagnosisIssue::error(
            "trader_decision", "action",
            "action not extracted from LLM response",
        ));
    }
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

pub fn validate_portfolio_decision(parsed: &super::GeneratedPortfolioDecision, raw: &str) -> Vec<DiagnosisIssue> {
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
    if parsed.rating == "Unknown" {
        issues.push(DiagnosisIssue::error(
            "portfolio_decision", "rating",
            "rating not extracted from LLM response",
        ));
    } else if parsed.rating == "Hold"
        && !raw.contains("rating")
        && !raw.contains("recommendation")
    {
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

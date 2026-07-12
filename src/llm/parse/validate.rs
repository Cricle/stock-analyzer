/// Validate a parsed research manager, returning issues for missing or placeholder fields.
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

/// Validate a parsed analyst decision, returning issues for missing or placeholder fields.
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
        && parsed.tool_calls.is_empty()
    {
        issues.push(DiagnosisIssue::error(
            "analyst_decision", "tool_name",
            "tool action but no tool_name or tool_calls",
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

/// Validate a parsed debate turn, returning issues for missing or placeholder fields.
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

/// Validate a parsed trader decision, checking required directional fields.
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
    // Check required fields for directional actions
    let is_directional = matches!(parsed.action.trim(), "Buy" | "Sell");
    if is_directional {
        let entry_empty = parsed.entry_price.as_ref().map(crate::llm::parse::normalize_value).unwrap_or_default().trim().is_empty();
        let stop_empty = parsed.stop_loss.as_ref().map(crate::llm::parse::normalize_value).unwrap_or_default().trim().is_empty();
        let horizon_empty = parsed.time_horizon.as_deref().unwrap_or("").trim().is_empty();
        if entry_empty {
            issues.push(DiagnosisIssue::error(
                "trader_decision", "entry_price",
                "entry_price is required for Buy/Sell but was empty or null",
            ));
        }
        if stop_empty {
            issues.push(DiagnosisIssue::error(
                "trader_decision", "stop_loss",
                "stop_loss is required for Buy/Sell but was empty or null",
            ));
        }
        if horizon_empty {
            issues.push(DiagnosisIssue::error(
                "trader_decision", "time_horizon",
                "time_horizon is required for Buy/Sell but was empty or null",
            ));
        }
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

/// Validate a parsed portfolio decision, checking required directional fields.
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
    // Check required fields for directional ratings
    let is_directional = matches!(parsed.rating.trim(), "Buy" | "Overweight" | "Underweight" | "Sell");
    if is_directional {
        let price_target_empty = parsed.price_target.as_ref().map(crate::llm::parse::normalize_value).unwrap_or_default().trim().is_empty();
        let horizon_empty = parsed.time_horizon.as_deref().unwrap_or("").trim().is_empty();
        let confirmation_empty = parsed.confirmation_level.as_ref().map(crate::llm::parse::normalize_value).unwrap_or_default().trim().is_empty();
        let invalidation_empty = parsed.invalidation_level.as_ref().map(crate::llm::parse::normalize_value).unwrap_or_default().trim().is_empty();
        if price_target_empty {
            issues.push(DiagnosisIssue::error(
                "portfolio_decision", "price_target",
                "price_target is required for directional rating but was empty or null",
            ));
        }
        if horizon_empty {
            issues.push(DiagnosisIssue::error(
                "portfolio_decision", "time_horizon",
                "time_horizon is required for directional rating but was empty or null",
            ));
        }
        if confirmation_empty {
            issues.push(DiagnosisIssue::error(
                "portfolio_decision", "confirmation_level",
                "confirmation_level is required for directional rating but was empty or null",
            ));
        }
        if invalidation_empty {
            issues.push(DiagnosisIssue::error(
                "portfolio_decision", "invalidation_level",
                "invalidation_level is required for directional rating but was empty or null",
            ));
        }
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

pub(crate) fn parse_generated_research_manager(
    content: &str,
) -> anyhow::Result<GeneratedResearchManager> {
    let parsed = parse_object_candidates_value(content, GeneratedResearchManager::from_value)?;
    validate_research_manager(&parsed, content);
    Ok(parsed)
}

pub(crate) fn parse_generated_portfolio_decision(
    content: &str,
) -> anyhow::Result<GeneratedPortfolioDecision> {
    let parsed = parse_object_candidates_value(content, GeneratedPortfolioDecision::from_value)?;
    validate_portfolio_decision(&parsed, content);
    Ok(parsed)
}

pub(crate) fn parse_generated_analyst_decision(
    content: &str,
) -> anyhow::Result<GeneratedAnalystDecision> {
    let parsed = parse_object_candidates_value(content, GeneratedAnalystDecision::from_value)?;
    if parsed.action.eq_ignore_ascii_case("finalize") && parsed.final_report.is_none() {
        let report = parse_object_candidates_value(content, GeneratedRoleReport::from_value)?;
        return Ok(GeneratedAnalystDecision {
            action: "finalize".to_string(),
            reasoning: "normalized legacy role-report response into analyst finalize decision"
                .to_string(),
            reasoning_key: None,
            final_report: Some(report),
            tool_name: None,
            tool_arguments: None,
        });
    }
    validate_analyst_decision(&parsed, content);
    Ok(parsed)
}

pub(crate) fn parse_generated_debate_turn(content: &str) -> anyhow::Result<GeneratedDebateTurn> {
    match parse_object_candidates_value(content, GeneratedDebateTurn::from_value) {
        Ok(parsed) => {
            validate_debate_turn(&parsed, content);
            Ok(parsed)
        }
        Err(primary_error) => parse_generated_debate_turn_lenient(content).ok_or(primary_error),
    }
}

pub(crate) fn parse_generated_trader_decision(
    content: &str,
) -> anyhow::Result<GeneratedTraderDecision> {
    let parsed = parse_object_candidates_value(content, GeneratedTraderDecision::from_value)?;
    validate_trader_decision(&parsed, content);
    Ok(parsed)
}

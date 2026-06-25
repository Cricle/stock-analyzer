pub fn derive_setup_tags(
    confidence_breakdown: &ConfidenceBreakdown,
    direction_breakdown: &DirectionBreakdown,
    execution_readiness: &ExecutionReadiness,
    research_plan: &StructuredResearchPlan,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> Vec<String> {
    let mut tags = Vec::new();

    if confidence_breakdown.trend_confirmation.score >= 12
        && direction_breakdown.market.score.abs() >= 10
    {
        tags.push("trend_confirmed".to_string());
    }
    if confidence_breakdown.catalyst_quality.score >= 8
        || !portfolio_decision.trigger_checklist.is_empty()
        || !trader_plan.execution_trigger_checklist.is_empty()
    {
        tags.push("event_driven".to_string());
    }
    if confidence_breakdown.fundamental_confirmation.score >= 12 {
        tags.push("fundamental_quality".to_string());
    }
    if !research_plan
        .missing_evidence_ladder
        .manageable_gaps
        .is_empty()
        || !portfolio_decision
            .missing_evidence_ladder
            .blocking_gaps
            .is_empty()
    {
        tags.push("valuation_sensitive".to_string());
    }
    if execution_readiness.execution_boundary_complete {
        tags.push("execution_ready".to_string());
    }
    if !execution_readiness.execution_boundary_complete {
        tags.push("watchlist_only".to_string());
    }

    let mut ordered = Vec::new();
    for tag in tags {
        if !ordered.iter().any(|existing: &String| existing == &tag) {
            ordered.push(tag);
        }
    }
    ordered
}

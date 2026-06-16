
fn allows_probe_position_before_confirmation(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> bool {
    let has_entry = !trader_plan.entry_price.trim().is_empty();
    let has_stop = !trader_plan.stop_loss.trim().is_empty();
    let has_confirmation = !portfolio_decision.confirmation_level.trim().is_empty()
        || !trader_plan.confirmation_level.trim().is_empty();
    let has_horizon = !portfolio_decision.time_horizon.trim().is_empty();
    let has_invalidation = !portfolio_decision.invalidation_level.trim().is_empty()
        || !trader_plan.stop_loss.trim().is_empty();
    let has_target = !portfolio_decision.target_reference.trim().is_empty()
        || !trader_plan.target_reference.trim().is_empty()
        || !trader_plan.target_condition.trim().is_empty();

    has_entry && has_stop && has_confirmation && has_horizon && has_invalidation && has_target
}

fn infer_target_condition(
    portfolio_decision: &StructuredPortfolioDecision,
    execution_boundary_complete: bool,
) -> String {
    if execution_boundary_complete || portfolio_decision.confirmation_level.trim().is_empty() {
        return String::new();
    }
    format!(
        "仅在价格有效处理并站稳 {} 后，该目标参考才具备执行意义。",
        portfolio_decision.confirmation_level.trim()
    )
}

fn infer_thesis_state(rating: Rating) -> ThesisState {
    if rating.is_bearish() {
        ThesisState::Weakening
    } else {
        ThesisState::Intact
    }
}


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

fn infer_target_type(
    portfolio_decision: &StructuredPortfolioDecision,
    execution_boundary_complete: bool,
) -> DecisionTargetType {
    let target = portfolio_decision.target_reference.trim();
    if target.is_empty() && execution_boundary_complete {
        return DecisionTargetType::Open;
    }
    if target.is_empty() {
        return DecisionTargetType::Unknown;
    }
    if target.contains('-') || target.contains('~') || target.contains("至") {
        return DecisionTargetType::Range;
    }
    if !execution_boundary_complete && !portfolio_decision.confirmation_level.trim().is_empty() {
        return DecisionTargetType::Conditional;
    }
    DecisionTargetType::Point
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

fn infer_timeframe(value: &str) -> DecisionTimeframe {
    if value.contains("周") || value.to_ascii_lowercase().contains("week") {
        DecisionTimeframe::ShortTerm
    } else if value.contains("月") || value.to_ascii_lowercase().contains("month") {
        DecisionTimeframe::Swing
    } else if value.contains("季") || value.to_ascii_lowercase().contains("quarter") {
        DecisionTimeframe::Position
    } else {
        DecisionTimeframe::Unknown
    }
}

fn infer_thesis_state(rating: Rating) -> ThesisState {
    if rating.is_bearish() {
        ThesisState::Weakening
    } else {
        ThesisState::Intact
    }
}

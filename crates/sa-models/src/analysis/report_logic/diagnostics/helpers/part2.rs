
fn nearest_anchor_above(current_price: Option<f64>, anchors: &[f64]) -> Option<f64> {
    let current = current_price?;
    anchors
        .iter()
        .copied()
        .filter(|anchor| *anchor > current * 1.01)
        .min_by(|left, right| {
            left.partial_cmp(right)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn format_price_reference(value: f64) -> String {
    if value >= 100.0 {
        format!("{value:.2}")
    } else {
        format!("{value:.4}")
    }
}

fn collect_missing_execution_fields(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> Vec<String> {
    let mut missing = Vec::new();
    if trader_plan.entry_price.trim().is_empty() {
        missing.push("entry_price".to_string());
    }
    if trader_plan.stop_loss.trim().is_empty() {
        missing.push("stop_loss".to_string());
    }
    if portfolio_decision.price_target.trim().is_empty()
        && portfolio_decision.confirmation_level.trim().is_empty()
    {
        missing.push("price_target".to_string());
    }
    if portfolio_decision.time_horizon.trim().is_empty() {
        missing.push("time_horizon".to_string());
    }
    missing
}

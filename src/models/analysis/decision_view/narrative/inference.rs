
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trader_plan(entry: &str, stop: &str, confirm: &str, target_ref: &str, target_cond: &str, horizon: &str) -> StructuredTraderPlan {
        StructuredTraderPlan {
            entry_price: entry.to_string(),
            stop_loss: stop.to_string(),
            confirmation_level: confirm.to_string(),
            target_reference: target_ref.to_string(),
            target_condition: target_cond.to_string(),
            time_horizon: horizon.to_string(),
            ..Default::default()
        }
    }

    fn make_portfolio(confirm: &str, horizon: &str, invalidation: &str, target_ref: &str) -> StructuredPortfolioDecision {
        StructuredPortfolioDecision {
            confirmation_level: confirm.to_string(),
            time_horizon: horizon.to_string(),
            invalidation_level: invalidation.to_string(),
            target_reference: target_ref.to_string(),
            ..Default::default()
        }
    }

    // --- allows_probe_position_before_confirmation ---

    #[test]
    fn probe_position_all_fields_present() {
        let tp = make_trader_plan("100", "95", "105", "120", "", "3个月");
        let pd = make_portfolio("105", "3个月", "95", "120");
        assert!(allows_probe_position_before_confirmation(&tp, &pd));
    }

    #[test]
    fn probe_position_missing_entry() {
        let tp = make_trader_plan("", "95", "105", "120", "", "3个月");
        let pd = make_portfolio("105", "3个月", "95", "120");
        assert!(!allows_probe_position_before_confirmation(&tp, &pd));
    }

    #[test]
    fn probe_position_missing_stop() {
        let tp = make_trader_plan("100", "", "105", "120", "", "3个月");
        let pd = make_portfolio("105", "3个月", "95", "120");
        assert!(!allows_probe_position_before_confirmation(&tp, &pd));
    }

    #[test]
    fn probe_position_missing_confirmation() {
        let tp = make_trader_plan("100", "95", "", "120", "", "3个月");
        let pd = make_portfolio("", "3个月", "95", "120");
        assert!(!allows_probe_position_before_confirmation(&tp, &pd));
    }

    #[test]
    fn probe_position_missing_horizon() {
        let tp = make_trader_plan("100", "95", "105", "120", "", "");
        let pd = make_portfolio("105", "", "95", "120");
        assert!(!allows_probe_position_before_confirmation(&tp, &pd));
    }

    #[test]
    fn probe_position_target_from_condition() {
        let tp = make_trader_plan("100", "95", "105", "", "站稳110", "3个月");
        let pd = make_portfolio("105", "3个月", "95", "");
        assert!(allows_probe_position_before_confirmation(&tp, &pd));
    }

    // --- infer_target_type ---

    #[test]
    fn target_type_empty_with_boundary_complete() {
        let pd = StructuredPortfolioDecision { target_reference: "".into(), ..Default::default() };
        assert_eq!(infer_target_type(&pd, true), DecisionTargetType::Open);
    }

    #[test]
    fn target_type_empty_without_boundary() {
        let pd = StructuredPortfolioDecision { target_reference: "".into(), ..Default::default() };
        assert_eq!(infer_target_type(&pd, false), DecisionTargetType::Unknown);
    }

    #[test]
    fn target_type_range_dash() {
        let pd = StructuredPortfolioDecision { target_reference: "100-120".into(), ..Default::default() };
        assert_eq!(infer_target_type(&pd, false), DecisionTargetType::Range);
    }

    #[test]
    fn target_type_range_tilde() {
        let pd = StructuredPortfolioDecision { target_reference: "100~120".into(), ..Default::default() };
        assert_eq!(infer_target_type(&pd, false), DecisionTargetType::Range);
    }

    #[test]
    fn target_type_range_chinese() {
        let pd = StructuredPortfolioDecision { target_reference: "100至120".into(), ..Default::default() };
        assert_eq!(infer_target_type(&pd, false), DecisionTargetType::Range);
    }

    #[test]
    fn target_type_conditional() {
        let pd = StructuredPortfolioDecision {
            target_reference: "120".into(),
            confirmation_level: "105".into(),
            ..Default::default()
        };
        assert_eq!(infer_target_type(&pd, false), DecisionTargetType::Conditional);
    }

    #[test]
    fn target_type_point() {
        let pd = StructuredPortfolioDecision {
            target_reference: "120".into(),
            confirmation_level: "".into(),
            ..Default::default()
        };
        assert_eq!(infer_target_type(&pd, false), DecisionTargetType::Point);
    }

    // --- infer_target_condition ---

    #[test]
    fn target_condition_boundary_complete() {
        let pd = StructuredPortfolioDecision { confirmation_level: "105".into(), ..Default::default() };
        assert_eq!(infer_target_condition(&pd, true), "");
    }

    #[test]
    fn target_condition_empty_confirmation() {
        let pd = StructuredPortfolioDecision { confirmation_level: "".into(), ..Default::default() };
        assert_eq!(infer_target_condition(&pd, false), "");
    }

    #[test]
    fn target_condition_with_confirmation() {
        let pd = StructuredPortfolioDecision { confirmation_level: "105".into(), ..Default::default() };
        let result = infer_target_condition(&pd, false);
        assert!(result.contains("105"));
        assert!(result.contains("站稳"));
    }

    // --- infer_timeframe ---

    #[test]
    fn timeframe_week_chinese() {
        assert_eq!(infer_timeframe("2周"), DecisionTimeframe::ShortTerm);
    }

    #[test]
    fn timeframe_week_english() {
        assert_eq!(infer_timeframe("2 weeks"), DecisionTimeframe::ShortTerm);
    }

    #[test]
    fn timeframe_month_chinese() {
        assert_eq!(infer_timeframe("3个月"), DecisionTimeframe::Swing);
    }

    #[test]
    fn timeframe_month_english() {
        assert_eq!(infer_timeframe("3 months"), DecisionTimeframe::Swing);
    }

    #[test]
    fn timeframe_quarter_chinese() {
        assert_eq!(infer_timeframe("1个季度"), DecisionTimeframe::Position);
    }

    #[test]
    fn timeframe_quarter_english() {
        assert_eq!(infer_timeframe("1 quarter"), DecisionTimeframe::Position);
    }

    #[test]
    fn timeframe_unknown() {
        assert_eq!(infer_timeframe("long term"), DecisionTimeframe::Unknown);
    }

    // --- infer_thesis_state ---

    #[test]
    fn thesis_state_bearish_sell() {
        assert_eq!(infer_thesis_state(Rating::Sell), ThesisState::Weakening);
    }

    #[test]
    fn thesis_state_bearish_underweight() {
        assert_eq!(infer_thesis_state(Rating::Underweight), ThesisState::Weakening);
    }

    #[test]
    fn thesis_state_bullish() {
        assert_eq!(infer_thesis_state(Rating::Buy), ThesisState::Intact);
    }

    #[test]
    fn thesis_state_neutral() {
        assert_eq!(infer_thesis_state(Rating::Hold), ThesisState::Intact);
    }
}

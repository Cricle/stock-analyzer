use stock_analyzer::analysis::{
    CashFlowSubstituteEvidence, ConfirmationMode, DecisionViewDirection, ExecutionBoundary,
    ExecutionPrerequisite, StructuredReport,
};

#[test]
fn execution_boundary_serializes_typed_prerequisites_and_stages() {
    let boundary = ExecutionBoundary {
        direction: DecisionViewDirection::Bearish,
        confirmation_price: Some(44.50),
        entry_price: Some(44.28),
        stop_price: Some(48.71),
        stage_one_target: Some(38.90),
        final_target: Some(36.08),
        minimum_reward_risk: 2.0,
        actual_reward_risk: Some(2.0),
        active_execution_allowed: false,
        confirmation_mode: ConfirmationMode::DailyCloseWithVolume,
        prerequisites: vec![ExecutionPrerequisite::BorrowQuantity],
        cash_flow_substitute: CashFlowSubstituteEvidence {
            cash_balance: Some(1_250.0),
            net_debt: Some(420.0),
            short_debt_coverage: Some(1.8),
            operating_loss_trend: Some(-12.5),
            replaces_cash_flow: false,
        },
    };

    let value = serde_json::to_value(boundary).unwrap();
    assert_eq!(value["direction"], "bearish");
    assert_eq!(value["confirmation_mode"], "daily_close_with_volume");
    assert_eq!(value["confirmation_price"], 44.50);
    assert_eq!(value["entry_price"], 44.28);
    assert_eq!(value["stop_price"], 48.71);
    assert_eq!(value["stage_one_target"], 38.90);
    assert_eq!(value["final_target"], 36.08);
    assert_eq!(value["minimum_reward_risk"], 2.0);
    assert_eq!(value["actual_reward_risk"], 2.0);
    assert_eq!(value["active_execution_allowed"], false);
    assert_eq!(value["prerequisites"][0], "borrow_quantity");
    assert_eq!(value["cash_flow_substitute"]["cash_balance"], 1_250.0);
    assert_eq!(value["cash_flow_substitute"]["replaces_cash_flow"], false);

    let report: StructuredReport = serde_json::from_str("{}").unwrap();
    assert_eq!(report.execution_boundary.direction, DecisionViewDirection::Neutral);
    assert!(report.execution_boundary.prerequisites.is_empty());
}

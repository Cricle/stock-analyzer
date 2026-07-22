use stock_analyzer::DecisionAction;

#[test]
fn conditional_bearish_action_serializes_as_wait_breakdown() {
    assert_eq!(
        serde_json::to_string(&DecisionAction::WaitBreakdown).unwrap(),
        "\"wait_breakdown\""
    );
}

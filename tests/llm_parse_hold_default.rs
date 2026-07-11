use stock_analyzer::llm::parse::{
    parse_generated_portfolio_decision, parse_generated_research_manager,
    parse_generated_trader_decision,
};

#[test]
fn research_manager_missing_recommendation_is_unknown() {
    let raw = r#"{"rationale": "test rationale", "risk_assessment": "low"}"#;
    let parsed = parse_generated_research_manager(raw).expect("should parse");
    assert_eq!(
        parsed.recommendation, "Unknown",
        "missing recommendation should default to Unknown, not Hold"
    );
}

#[test]
fn portfolio_decision_missing_rating_is_unknown() {
    let raw = r#"{"executive_summary": "test", "rationale": "test", "investment_thesis": "test"}"#;
    let parsed = parse_generated_portfolio_decision(raw).expect("should parse");
    assert_eq!(
        parsed.rating, "Unknown",
        "missing rating should default to Unknown, not Hold"
    );
}

#[test]
fn trader_decision_missing_action_is_unknown() {
    let raw = r#"{"reasoning": "test", "trader_plan": "test"}"#;
    let parsed = parse_generated_trader_decision(raw).expect("should parse");
    assert_eq!(
        parsed.action, "Unknown",
        "missing action should default to Unknown, not Hold"
    );
}

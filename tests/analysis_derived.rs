use sa::{AnalysisResult, LocalText, Rating};

fn make_result() -> AnalysisResult {
    AnalysisResult {
        task_id: "test-task".to_string(),
        report_id: "test-report".to_string(),
        symbol: "AAPL".to_string(),
        stock_name: "Apple".to_string(),
        analysis_date: "2025-01-15".to_string(),
        market_type: "美股".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: Default::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2025-01-15T00:00:00Z".to_string(),
    }
}

// --- derived_summary ---

#[test]
fn derived_summary_from_portfolio_decision() {
    let mut result = make_result();
    result
        .agent_state
        .structured_portfolio_decision
        .executive_summary = LocalText::new("Strong buy recommendation");
    assert_eq!(result.derived_summary(), "Strong buy recommendation");
}

#[test]
fn derived_summary_from_research_plan() {
    let mut result = make_result();
    result.agent_state.structured_research_plan.rationale = LocalText::new("Research rationale");
    assert_eq!(result.derived_summary(), "Research rationale");
}

#[test]
fn derived_summary_default() {
    let result = make_result();
    let summary = result.derived_summary();
    assert!(summary.contains("AAPL"));
    assert!(summary.contains("2025-01-15"));
}

// --- derived_recommendation ---

#[test]
fn derived_recommendation_from_portfolio_decision() {
    let mut result = make_result();
    result.agent_state.structured_portfolio_decision.rating = Rating::Buy;
    result.agent_state.structured_portfolio_decision.raw_rating = "Buy".to_string();
    assert_eq!(result.derived_recommendation(), "Buy");
}

#[test]
fn derived_recommendation_from_research_plan() {
    let mut result = make_result();
    result.agent_state.structured_research_plan.recommendation = LocalText::new("Overweight");
    assert_eq!(result.derived_recommendation(), "Overweight");
}

#[test]
fn derived_recommendation_default_hold() {
    let result = make_result();
    assert_eq!(result.derived_recommendation(), "Hold");
}

// --- derived_risk_assessment ---

#[test]
fn derived_risk_assessment_from_portfolio_decision() {
    let mut result = make_result();
    result
        .agent_state
        .structured_portfolio_decision
        .risk_assessment = LocalText::new("High risk");
    assert_eq!(result.derived_risk_assessment(), "High risk");
}

#[test]
fn derived_risk_assessment_from_research_plan() {
    let mut result = make_result();
    result.agent_state.structured_research_plan.risk_assessment = LocalText::new("Moderate risk");
    assert_eq!(result.derived_risk_assessment(), "Moderate risk");
}

#[test]
fn derived_risk_assessment_default() {
    let result = make_result();
    assert_eq!(result.derived_risk_assessment(), "待分析");
}

// --- derived_confidence ---

#[test]
fn derived_confidence_from_portfolio_decision() {
    let mut result = make_result();
    result.agent_state.structured_portfolio_decision.confidence = LocalText::new("High confidence");
    assert_eq!(result.derived_confidence(), "High confidence");
}

#[test]
fn derived_confidence_from_research_plan() {
    let mut result = make_result();
    result.agent_state.structured_research_plan.confidence = LocalText::new("Medium");
    assert_eq!(result.derived_confidence(), "Medium");
}

// --- derived_rationale ---

#[test]
fn derived_rationale_from_portfolio_decision() {
    let mut result = make_result();
    result
        .agent_state
        .structured_portfolio_decision
        .investment_thesis = LocalText::new("Strong thesis");
    assert_eq!(result.derived_rationale(), "Strong thesis");
}

#[test]
fn derived_rationale_from_research_plan() {
    let mut result = make_result();
    result.agent_state.structured_research_plan.rationale = LocalText::new("Research rationale");
    assert_eq!(result.derived_rationale(), "Research rationale");
}

// --- report_stage ---

#[test]
fn report_stage_all_empty() {
    let result = make_result();
    let stage = result.report_stage();
    assert!(stage.overview);
    assert!(!stage.market);
    assert!(!stage.fundamentals);
    assert!(!stage.news);
    assert!(!stage.sentiment);
    assert!(!stage.bull_research);
    assert!(!stage.bear_research);
    assert!(!stage.research_plan);
    assert!(!stage.trader_plan);
    assert!(!stage.risk_debate);
    assert!(!stage.portfolio_decision);
    assert!(!stage.reflection);
}

#[test]
fn report_stage_partial() {
    let mut result = make_result();
    result.agent_state.market_report = "Market analysis".to_string();
    result.agent_state.fundamentals_report = "Fundamentals analysis".to_string();
    result.agent_state.final_trade_decision = "Buy".to_string();
    let stage = result.report_stage();
    assert!(stage.overview);
    assert!(stage.market);
    assert!(stage.fundamentals);
    assert!(!stage.news);
    assert!(stage.portfolio_decision);
}

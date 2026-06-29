use sa::llm::tools::{AnalysisDataCollector, execute_tool_call};
use serde_json::json;

#[test]
fn test_all_rating_tools() {
    let c = AnalysisDataCollector::new();
    assert!(execute_tool_call(&c, "set_rating", &json!({"rating": "Buy"})).is_ok());
    assert!(execute_tool_call(&c, "set_rating", &json!({"rating": "Invalid"})).is_err());
    assert!(execute_tool_call(&c, "set_confidence", &json!({"score": 75.0})).is_ok());
    assert!(execute_tool_call(&c, "set_confidence", &json!({"score": 150.0})).is_err());
}

#[test]
fn test_all_price_tools() {
    let c = AnalysisDataCollector::new();
    assert!(execute_tool_call(&c, "set_entry_price", &json!({"value": 100.0})).is_ok());
    assert!(execute_tool_call(&c, "set_stop_loss", &json!({"value": 95.0})).is_ok());
    assert!(execute_tool_call(&c, "set_target_price", &json!({"value": 120.0})).is_ok());
    assert!(execute_tool_call(&c, "set_entry_price", &json!({"value": -10.0})).is_err());
}

#[test]
fn test_all_text_tools() {
    let c = AnalysisDataCollector::new();
    assert!(execute_tool_call(&c, "set_executive_summary", &json!({"value": "summary"})).is_ok());
    assert!(execute_tool_call(&c, "set_rationale", &json!({"value": "rationale"})).is_ok());
    assert!(execute_tool_call(&c, "set_rationale", &json!({"value": ""})).is_err());
}

#[test]
fn test_all_evidence_tools() {
    let c = AnalysisDataCollector::new();
    assert!(execute_tool_call(&c, "add_evidence_point", &json!({"value": "evidence"})).is_ok());
    assert!(execute_tool_call(&c, "add_key_risk", &json!({"value": "risk"})).is_ok());
    assert!(execute_tool_call(&c, "add_trigger", &json!({"value": "trigger"})).is_ok());
    assert!(execute_tool_call(&c, "add_blocking_gap", &json!({"value": "gap"})).is_ok());
}

#[test]
fn test_probability_and_score() {
    let c = AnalysisDataCollector::new();
    assert!(execute_tool_call(&c, "set_probability", &json!({"up": 0.5, "down": 0.3, "sideways": 0.2})).is_ok());
    assert!(execute_tool_call(&c, "set_score", &json!({"dimension": "direction", "score": 75.0})).is_ok());
}

#[test]
fn test_scenario_path() {
    let c = AnalysisDataCollector::new();
    assert!(execute_tool_call(&c, "add_scenario_path", &json!({
        "key": "breakout",
        "name": "Breakout",
        "action": "buy"
    })).is_ok());
    assert_eq!(c.build().scenario_paths.len(), 1);
}

#[test]
fn test_meta_tools() {
    let c = AnalysisDataCollector::new();
    assert!(execute_tool_call(&c, "set_time_horizon", &json!({"value": "2-6 weeks"})).is_ok());
    assert!(execute_tool_call(&c, "set_time_stop", &json!({"deadline": "10 days", "reason": "exit"})).is_ok());
    assert!(execute_tool_call(&c, "set_reflection", &json!({
        "strongest_part": "strong",
        "weakest_uncertainty": "weak",
        "next_lesson": "lesson"
    })).is_ok());
}

#[test]
fn test_debate_tools() {
    let c = AnalysisDataCollector::new();
    assert!(execute_tool_call(&c, "set_speaker", &json!({"value": "Bull"})).is_ok());
    assert!(execute_tool_call(&c, "set_stance", &json!({"value": "bull"})).is_ok());
    assert!(execute_tool_call(&c, "set_response", &json!({"value": "response"})).is_ok());
}

#[test]
fn test_unknown_tool() {
    let c = AnalysisDataCollector::new();
    assert!(execute_tool_call(&c, "unknown", &json!({})).is_err());
}

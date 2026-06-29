use sa::llm::GeneratedResearchManager;
use sa::llm::parse::*;

#[test]
fn repairs_research_manager_missing_object_wrapper() {
    let content = r#"{"recommendation":"Overweight","rating":"Overweight","confidence":0.66,"risk_assessment":{"overall_risk_frame":"frame","key_risks":["risk"],"offsetting_supports":["support"],"missing_evidence":"tolerable_context_gaps":["gap-a"],"serious_but_manageable_gaps":["gap-b"],"decision_blocking_gaps":["gap-c"]},"rationale":"because","strategic_actions":{"position_expression":"small","what_to_monitor_next":["223.75"]}}"#;

    let parsed = parse_generated_research_manager(content).expect("should parse repaired JSON");

    assert_eq!(parsed.recommendation, "Overweight");
    assert!(parsed.risk_assessment.contains("overall_risk_frame: frame"));
    assert!(
        parsed
            .risk_assessment
            .contains("missing_evidence: tolerable_context_gaps: gap-a")
    );
    assert!(
        parsed
            .risk_assessment
            .contains("serious_but_manageable_gaps: gap-b")
    );
    assert_eq!(
        parsed.strategic_actions,
        "position_expression: small\nwhat_to_monitor_next: 223.75"
    );
}

#[test]
fn leniently_parses_debate_turn_with_unescaped_quotes_in_response() {
    let content = r#"{"speaker":"Bear Researcher","stance":"bear","response":"我不同意多头把NVDA视为"只会越来越强"的前提。问题不在于公司差，而在于市场把"AI平台"叙事过度外推。","confidence":0.81,"evidence_points":["估值对持续超预期依赖很强","趋势拥挤提高回撤脆弱性"],"risks":["若后续财报继续大超预期，空头判断会被证伪"]}"#;

    let parsed = parse_generated_debate_turn(content).expect("should parse lenient debate JSON");

    assert_eq!(parsed.speaker, "Bear Researcher");
    assert_eq!(parsed.stance, "bear");
    assert!(parsed.response.contains("只会越来越强"));
    assert_eq!(parsed.evidence_points.len(), 2);
    assert_eq!(parsed.risks.len(), 1);
}

#[test]
fn sanitize_json_control_chars_escapes_newline_in_string() {
    let input = r#"{"key":"line1\nline2"}"#;
    let result = sanitize_json_control_chars(input);
    assert!(result.contains("\\n"));
    let _: serde_json::Value = serde_json::from_str(&result).unwrap();
}

#[test]
fn sanitize_json_control_chars_escapes_tab() {
    let input = "{\"key\":\"col1\tcol2\"}";
    let result = sanitize_json_control_chars(input);
    assert!(result.contains("\\t"));
    let _: serde_json::Value = serde_json::from_str(&result).unwrap();
}

#[test]
fn sanitize_json_control_chars_preserves_escaped_quotes() {
    let input = r#"{"key":"value with \"quote\""}"#;
    let result = sanitize_json_control_chars(input);
    assert!(result.contains(r#"\"quote\""#));
}

#[test]
fn sanitize_json_control_chars_no_change_needed() {
    let input = r#"{"key":"clean value"}"#;
    let result = sanitize_json_control_chars(input);
    assert_eq!(result, input);
}

#[test]
fn candidate_variants_includes_trimmed_and_fenced() {
    let content = "```json\n{\"key\":1}\n```";
    let variants = candidate_variants(content);
    assert!(variants.len() >= 2);
    assert!(variants.iter().any(|v| v.contains("{\"key\":1}")));
}

#[test]
fn candidate_variants_plain_json() {
    let content = r#"{"key": 1}"#;
    let variants = candidate_variants(content);
    assert!(!variants.is_empty());
}

#[test]
fn text_or_default_returns_default_for_none() {
    assert_eq!(text_or_default(None, "fallback"), "fallback");
}

#[test]
fn text_or_default_returns_value_for_some() {
    assert_eq!(
        text_or_default(Some(serde_json::json!("hello")), "fallback"),
        "hello"
    );
}

#[test]
fn text_or_default_returns_default_for_empty() {
    assert_eq!(
        text_or_default(Some(serde_json::json!("")), "fallback"),
        "fallback"
    );
}

#[test]
fn first_non_empty_returns_first_non_empty() {
    let a = serde_json::json!("first");
    let b = serde_json::json!("second");
    assert_eq!(first_non_empty(&[Some(&a), Some(&b)], "default"), "first");
}

#[test]
fn first_non_empty_skips_empty() {
    let empty = serde_json::json!("");
    let valid = serde_json::json!("valid");
    assert_eq!(
        first_non_empty(&[Some(&empty), Some(&valid)], "default"),
        "valid"
    );
}

#[test]
fn first_non_empty_returns_default_when_all_none() {
    assert_eq!(first_non_empty(&[None, None], "default"), "default");
}

#[test]
fn string_list_or_default_returns_defaults_for_none() {
    let result = string_list_or_default(None, &["a", "b"]);
    assert_eq!(result, vec!["a", "b"]);
}

#[test]
fn string_list_or_default_parses_array() {
    let value = serde_json::json!(["x", "y", "z"]);
    let result = string_list_or_default(Some(value), &["default"]);
    assert_eq!(result, vec!["x", "y", "z"]);
}

#[test]
fn string_list_or_default_returns_defaults_for_empty_array() {
    let value = serde_json::json!([]);
    let result = string_list_or_default(Some(value), &["default"]);
    assert_eq!(result, vec!["default"]);
}

#[test]
fn string_list_or_default_filters_empty_strings() {
    let value = serde_json::json!(["a", "", "b"]);
    let result = string_list_or_default(Some(value), &["default"]);
    assert_eq!(result, vec!["a", "b"]);
}

#[test]
fn parse_portfolio_decision_valid_json() {
    let content = r#"{"rating":"Buy","confidence":0.8,"risk_assessment":"moderate risk","summary":"good entry","rationale":"strong fundamentals","executive_summary":"Buy NVDA","investment_thesis":"AI growth"}"#;
    let result = parse_generated_portfolio_decision(content);
    assert!(result.is_ok());
    let decision = result.unwrap();
    assert_eq!(decision.rating, "Buy");
}

#[test]
fn parse_portfolio_decision_with_fenced_json() {
    let content = "```json\n{\"rating\":\"Hold\",\"confidence\":0.5,\"risk_assessment\":\"low\",\"summary\":\"wait\",\"rationale\":\"uncertain\",\"executive_summary\":\"Hold\",\"investment_thesis\":\"wait\"}\n```";
    let result = parse_generated_portfolio_decision(content);
    assert!(result.is_ok());
}

#[test]
fn parse_analyst_decision_valid_json() {
    let content = r#"{"action":"tool","reasoning":"need data","tool_name":"get_stock_data","tool_arguments":"{}"}"#;
    let result = parse_generated_analyst_decision(content);
    assert!(result.is_ok());
    let decision = result.unwrap();
    assert_eq!(decision.action, "tool");
    assert_eq!(decision.tool_name.as_deref(), Some("get_stock_data"));
}

#[test]
fn parse_trader_decision_valid_json() {
    let content = r#"{"action":"buy","trader_plan":"enter at 100","reasoning":"support level"}"#;
    let result = parse_generated_trader_decision(content);
    assert!(result.is_ok());
    let decision = result.unwrap();
    assert_eq!(decision.action, "buy");
}

#[test]
fn parse_subscription_qa_answer_valid() {
    let content = r#"{"answer":"Yes","confidence":0.9}"#;
    let result = parse_generated_subscription_qa_answer(content);
    assert!(result.is_ok());
}

#[test]
fn validate_research_manager_flags_default_rationale() {
    let parsed: GeneratedResearchManager = serde_json::from_str(
        r#"{"recommendation":"Buy","confidence":0.8,"risk_assessment":"real","rationale":"模型未返回研究经理结论。","strategic_actions":"actions"}"#,
    ).unwrap();
    let issues = validate_research_manager(&parsed, "{}");
    assert!(issues.iter().any(|i| i.field == "rationale"));
}

#[test]
fn parse_analyst_decision_batch_tool_calls() {
    let content = r#"{
        "action": "tool",
        "reasoning": "need multiple data points",
        "tool_calls": [
            {"tool_name": "get_stock_data", "tool_arguments": {}},
            {"tool_name": "get_indicators", "tool_arguments": {}}
        ]
    }"#;
    let result = parse_generated_analyst_decision(content);
    assert!(result.is_ok());
    let decision = result.unwrap();
    assert_eq!(decision.action, "tool");
    assert_eq!(decision.tool_calls.len(), 2);
    assert_eq!(decision.tool_calls[0].tool_name, "get_stock_data");
    assert_eq!(decision.tool_calls[1].tool_name, "get_indicators");
}

#[test]
fn parse_analyst_decision_single_tool_backward_compat() {
    let content = r#"{"action":"tool","reasoning":"need data","tool_name":"get_stock_data","tool_arguments":"{}"}"#;
    let result = parse_generated_analyst_decision(content);
    assert!(result.is_ok());
    let decision = result.unwrap();
    assert_eq!(decision.tool_name.as_deref(), Some("get_stock_data"));
    assert!(decision.tool_calls.is_empty());
}

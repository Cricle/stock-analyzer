use stock_analyzer::scoring::dimensions::sentiment::parse_sentiment_response;

#[test]
fn test_parse_valid_json() {
    let raw = r#"{"score": 75, "reason": "近期利好消息较多"}"#;
    let result = parse_sentiment_response(raw);
    assert_eq!(result.score, 75);
    assert!(result.reason.contains("利好"));
}

#[test]
fn test_parse_json_in_codeblock() {
    let raw = "```json\n{\"score\": 30, \"reason\": \"利空\"}\n```";
    let result = parse_sentiment_response(raw);
    assert_eq!(result.score, 30);
}

#[test]
fn test_parse_invalid_returns_neutral() {
    let raw = "I cannot provide a score";
    let result = parse_sentiment_response(raw);
    assert_eq!(result.score, 50);
}

#[test]
fn test_parse_score_over_100_clamped() {
    let raw = r#"{"score": 150, "reason": "超出范围"}"#;
    let result = parse_sentiment_response(raw);
    assert_eq!(result.score, 100, "score should be clamped to 100");
}

#[test]
fn test_parse_score_under_0_clamped() {
    let raw = r#"{"score": 0, "reason": "最低分"}"#;
    let result = parse_sentiment_response(raw);
    assert_eq!(result.score, 0);
}

#[test]
fn test_parse_json_with_extra_whitespace() {
    let raw = "  \n  {\"score\": 60, \"reason\": \"偏积极\"}  \n  ";
    let result = parse_sentiment_response(raw);
    assert_eq!(result.score, 60);
}

#[test]
fn test_parse_json_with_trailing_text() {
    let raw = r#"{"score": 40, "reason": "偏消极"} some extra text"#;
    let result = parse_sentiment_response(raw);
    // Should fail to parse and return neutral
    assert_eq!(result.score, 50);
}

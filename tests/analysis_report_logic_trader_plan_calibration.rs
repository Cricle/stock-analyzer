use stock_analyzer::analysis::{
    StructuredRiskAssessment, first_non_empty_sentence, is_semantically_similar,
    normalize_semantic_snippet, parse_risk_assessment_sections, split_semicolon_items,
    strip_redundant_prefix,
};

#[test]
fn first_non_empty_sentence_basic() {
    let result = first_non_empty_sentence(&["", "hello world", "other"]);
    assert_eq!(result, Some("hello world".into()));
}

#[test]
fn first_non_empty_sentence_all_empty() {
    let result = first_non_empty_sentence(&["", "  "]);
    assert!(result.is_none());
}

#[test]
fn first_non_empty_sentence_multiline() {
    let result = first_non_empty_sentence(&["line1\nline2"]);
    assert_eq!(result, Some("line1".into()));
}

#[test]
fn strip_redundant_prefix_basic() {
    let result = strip_redundant_prefix("Recommendation: Buy", &["Recommendation"]);
    assert_eq!(result, "Buy");
}

#[test]
fn strip_redundant_prefix_no_match() {
    let result = strip_redundant_prefix("Buy", &["Recommendation"]);
    assert_eq!(result, "Buy");
}

#[test]
fn strip_redundant_prefix_multiple() {
    let result = strip_redundant_prefix(
        "Action: Recommendation: Buy",
        &["Action:", "Recommendation"],
    );
    assert_eq!(result, "Buy");
}

#[test]
fn split_semicolon_items_basic() {
    let result = split_semicolon_items("a; b; c");
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[test]
fn split_semicolon_items_empty() {
    let result = split_semicolon_items("");
    assert!(result.is_empty());
}

#[test]
fn split_semicolon_items_whitespace() {
    let result = split_semicolon_items("  a  ;  b  ");
    assert_eq!(result, vec!["a", "b"]);
}

#[test]
fn normalize_semantic_snippet_basic() {
    let result = normalize_semantic_snippet("Hello World!");
    assert_eq!(result, "hello world");
}

#[test]
fn normalize_semantic_snippet_chinese() {
    let result = normalize_semantic_snippet("你好世界");
    assert_eq!(result, "你好世界");
}

#[test]
fn normalize_semantic_snippet_mixed() {
    let result = normalize_semantic_snippet("AAPL股价上涨");
    assert_eq!(result, "aapl股价上涨");
}

#[test]
fn is_semantically_similar_same() {
    let left = Some(&"hello world".to_string());
    let right = Some(&"hello world".to_string());
    assert!(is_semantically_similar(left, right));
}

#[test]
fn is_semantically_similar_subset() {
    let left = Some(&"hello world foo".to_string());
    let right = Some(&"hello world".to_string());
    assert!(is_semantically_similar(left, right));
}

#[test]
fn is_semantically_similar_different() {
    let left = Some(&"hello".to_string());
    let right = Some(&"world".to_string());
    assert!(!is_semantically_similar(left, right));
}

#[test]
fn is_semantically_similar_none() {
    assert!(!is_semantically_similar(None, None));
}

#[test]
fn parse_risk_assessment_sections_empty() {
    let sections = parse_risk_assessment_sections("");
    assert!(sections.is_empty());
}

#[test]
fn parse_risk_assessment_sections_basic() {
    let text = "key_risks: market volatility\noffsetting_supports: strong fundamentals";
    let sections = parse_risk_assessment_sections(text);
    assert!(sections.contains_key("key_risks"));
    assert!(sections.contains_key("offsetting_supports"));
}

#[test]
fn structured_risk_assessment_from_text_empty() {
    let assessment = StructuredRiskAssessment::from_text("");
    assert!(assessment.overall_risk_framing.is_empty());
}

#[test]
fn structured_risk_assessment_from_text_plain() {
    let assessment = StructuredRiskAssessment::from_text("market risk is moderate");
    assert_eq!(assessment.overall_risk_framing, "market risk is moderate");
    assert!(assessment.key_risks.is_empty());
}

#[test]
fn structured_risk_assessment_from_text_structured() {
    let text = "overall_risk_framing: moderate risk\nkey_risks: volatility; liquidity";
    let assessment = StructuredRiskAssessment::from_text(text);
    assert_eq!(assessment.overall_risk_framing, "moderate risk");
    assert_eq!(assessment.key_risks.len(), 2);
}

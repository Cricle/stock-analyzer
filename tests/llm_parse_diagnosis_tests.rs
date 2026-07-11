use stock_analyzer::llm::parse::{DiagnosisIssue, IssueSeverity};

#[test]
fn diagnosis_issue_error() {
    let issue = DiagnosisIssue::error("content", "summary", "too short");
    assert!(matches!(issue.severity, IssueSeverity::Error));
    assert_eq!(issue.category, "content");
    assert_eq!(issue.field, "summary");
    assert_eq!(issue.message, "too short");
}

#[test]
fn diagnosis_issue_warning() {
    let issue = DiagnosisIssue::warning("format", "json", "malformed");
    assert!(matches!(issue.severity, IssueSeverity::Warning));
    assert_eq!(issue.category, "format");
}

#[test]
fn diagnosis_issue_info() {
    let issue = DiagnosisIssue::info("quality", "rationale", "could be longer");
    assert!(matches!(issue.severity, IssueSeverity::Info));
}

#[test]
fn diagnosis_issue_from_strings() {
    let issue = DiagnosisIssue::error(
        "test_category".to_string(),
        "test_field".to_string(),
        "test_message".to_string(),
    );
    assert_eq!(issue.category, "test_category");
    assert_eq!(issue.field, "test_field");
    assert_eq!(issue.message, "test_message");
}

use sa::llm::parse::DiagnosisIssue;
use sa::llm::retry::default_retry_hint_builder;

#[test]
fn default_retry_hint_builder_single_issue() {
    let issues = vec![DiagnosisIssue::error("content", "summary", "empty field")];
    let hint = default_retry_hint_builder(&issues, 1);
    assert!(hint.contains("retry 1"));
    assert!(hint.contains("summary: empty field"));
    assert!(hint.contains("strict JSON"));
}

#[test]
fn default_retry_hint_builder_multiple_issues() {
    let issues = vec![
        DiagnosisIssue::error("content", "summary", "too short"),
        DiagnosisIssue::error("content", "rating", "invalid value"),
    ];
    let hint = default_retry_hint_builder(&issues, 2);
    assert!(hint.contains("retry 2"));
    assert!(hint.contains("summary: too short"));
    assert!(hint.contains("rating: invalid value"));
}

#[test]
fn default_retry_hint_builder_empty_issues() {
    let hint = default_retry_hint_builder(&[], 1);
    assert!(hint.contains("retry 1"));
    assert!(hint.contains("quality issues: "));
}

#[test]
fn default_retry_hint_builder_high_retry_count() {
    let hint = default_retry_hint_builder(&[], 5);
    assert!(hint.contains("retry 5"));
}

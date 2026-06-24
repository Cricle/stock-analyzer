#[derive(Clone, Debug)]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug)]
pub struct DiagnosisIssue {
    pub severity: IssueSeverity,
    pub category: String,
    pub field: String,
    pub message: String,
}

impl DiagnosisIssue {
    pub fn error(
        category: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: IssueSeverity::Error,
            category: category.into(),
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn warning(
        category: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: IssueSeverity::Warning,
            category: category.into(),
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn info(
        category: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: IssueSeverity::Info,
            category: category.into(),
            field: field.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

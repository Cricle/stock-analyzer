/// Severity level for LLM output quality diagnosis issues.
#[derive(Clone, Debug)]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

/// A single diagnostic issue found during LLM output validation.
#[derive(Clone, Debug)]
pub struct DiagnosisIssue {
    pub severity: IssueSeverity,
    pub category: String,
    pub field: String,
    pub message: String,
}

impl DiagnosisIssue {
    /// Create an error-severity diagnosis issue.
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

    /// Create a warning-severity diagnosis issue.
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

    /// Create an info-severity diagnosis issue.
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

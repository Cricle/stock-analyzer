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
    pub fn error(category: impl Into<String>, field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Error,
            category: category.into(),
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn warning(category: impl Into<String>, field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Warning,
            category: category.into(),
            field: field.into(),
            message: message.into(),
        }
    }
}

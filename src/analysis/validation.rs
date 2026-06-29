use serde::{Deserialize, Serialize};

/// Result of running all validators on an LLM output.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the recommendation contradicts technical indicators.
    pub consistency_flag: bool,
    /// Reason for consistency flag (empty if not flagged).
    pub consistency_reason: String,
    /// Whether outputs are uniform across stocks in a batch.
    pub uniformity_flag: bool,
    /// Percentage of fields that are identical across stocks.
    pub uniformity_pct: f64,
    /// Missing execution boundary fields.
    pub missing_boundary_fields: Vec<String>,
    /// Confidence adjustment from validation (negative = reduce).
    pub confidence_adjustment: i32,
    /// Action score adjustment from validation (negative = reduce).
    pub action_adjustment: i32,
}

impl ValidationResult {
    /// Returns true if any validator flagged an issue.
    pub fn has_issues(&self) -> bool {
        self.consistency_flag || self.uniformity_flag || !self.missing_boundary_fields.is_empty()
    }
}

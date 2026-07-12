
/// Calibration bias — direction and magnitude adjustments.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CalibrationBias {
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub direction: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub magnitude: LocalText,
    #[serde(default, skip_serializing)]
    pub rationale: LocalText,
}

/// Diagnostic items collected during report generation.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportDiagnostics {
    #[serde(default)]
    pub market: Vec<ReportDiagnosticItem>,
    #[serde(default)]
    pub fundamentals: Vec<ReportDiagnosticItem>,
    #[serde(default)]
    pub news: Vec<ReportDiagnosticItem>,
    #[serde(default)]
    pub availability: Vec<ReportDiagnosticItem>,
}

/// A single diagnostic item with code, severity, and message.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportDiagnosticItem {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub message: LocalText,
    #[serde(default)]
    pub details: Vec<String>,
    #[serde(default)]
    pub related_blocking_gaps: Vec<String>,
    #[serde(default)]
    pub related_trigger_checklist: Vec<String>,
    #[serde(default)]
    pub elevated_to_execution_blocking_gap: bool,
}

/// Snapshot of reference facts used in the report.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportReferenceSnapshot {
    #[serde(default)]
    pub market: Vec<ReferenceFactItem>,
    #[serde(default)]
    pub fundamentals: Vec<ReferenceFactItem>,
    #[serde(default)]
    pub news: Vec<ReferenceFactItem>,
    #[serde(default)]
    pub memory: Vec<ReferenceFactItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Reference fact item.
pub struct ReferenceFactItem {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub emphasis: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub summary: String,
}

/// Historical calibration statistics (sample count, hit rate, avg return).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HistoricalCalibrationStats {
    #[serde(default)]
    pub sample_count: usize,
    #[serde(default)]
    pub hit_rate: f64,
    #[serde(default)]
    pub avg_alpha_return: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Confidence breakdown.
pub struct ConfidenceBreakdown {
    #[serde(default)]
    pub data_quality: ScoreDimension,
    #[serde(default)]
    pub trend_confirmation: ScoreDimension,
    #[serde(default)]
    pub fundamental_confirmation: ScoreDimension,
    #[serde(default)]
    pub catalyst_quality: ScoreDimension,
    #[serde(default)]
    pub historical_transferability: ScoreDimension,
    #[serde(default)]
    pub cross_agent_consistency: ScoreDimension,
    #[serde(default)]
    pub risk_clarity: ScoreDimension,
    #[serde(default)]
    pub total_before_caps: i32,
    #[serde(default)]
    pub final_score: i32,
    #[serde(default)]
    pub applied_cap: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Score dimension.
pub struct ScoreDimension {
    pub score: i32,
    pub max_score: i32,
    #[serde(default, skip_serializing)]
    pub rationale: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Confidence cap.
pub struct ConfidenceCap {
    pub key: String,
    #[serde(default, skip_serializing)]
    pub label: LocalText,
    pub cap: i32,
    #[serde(default, skip_serializing)]
    pub reason: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
/// Research reliability.
pub struct ResearchReliability {
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub max_score: i32,
    #[serde(default)]
    pub label: LocalText,
    #[serde(default, skip_serializing)]
    pub rationale: LocalText,
    #[serde(default)]
    pub strengths: Vec<LocalText>,
    #[serde(default)]
    pub constraints: Vec<LocalText>,
}

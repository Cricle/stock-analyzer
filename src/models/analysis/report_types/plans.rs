
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TradeSetupQuality {
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub max_score: i32,
    #[serde(default)]
    pub ready: bool,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub label: LocalText,
    #[serde(default, skip_serializing, deserialize_with = "deserialize_local_text_or_string")]
    pub rationale: LocalText,
    #[serde(default)]
    pub strengths: Vec<LocalText>,
    #[serde(default)]
    pub gaps: Vec<LocalText>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CalibrationSummary {
    #[serde(default)]
    pub threshold_tightened: bool,
    #[serde(default)]
    pub memory_threshold_tightened: bool,
    #[serde(default)]
    pub min_confidence_score: i32,
    #[serde(default)]
    pub min_action_score: i32,
    #[serde(default)]
    pub direction_floor_abs: i32,
    #[serde(default)]
    pub strong_direction_abs: i32,
    #[serde(default)]
    pub direction_threshold_penalty: i32,
    #[serde(default)]
    pub historical: HistoricalCalibrationStats,
    #[serde(default)]
    pub setup_calibration_sample_count: usize,
    #[serde(default)]
    pub setup_match_count: usize,
    #[serde(default)]
    pub setup_pending_match_count: usize,
    #[serde(default)]
    pub setup_match_explanation: SetupMatchExplanation,
    #[serde(default)]
    pub setup_match_quality: ScoreDimension,
    #[serde(default)]
    pub setup_direction_alignment: ScoreDimension,
    #[serde(default)]
    pub calibration_bias: CalibrationBias,
    #[serde(default, skip_serializing, deserialize_with = "deserialize_local_text_or_string")]
    pub decision_narrative: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SetupMatchExplanation {
    #[serde(default)]
    pub reason_code: String,
    #[serde(default, skip_serializing)]
    pub summary: String,
    #[serde(default, skip_serializing)]
    pub details: Vec<String>,
    #[serde(default)]
    pub fallback_used: bool,
    #[serde(default)]
    pub fallback_sample_count: usize,
}

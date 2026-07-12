/// Maximum score for data quality dimension.
pub const DATA_QUALITY_MAX: i32 = 25;
/// Maximum score for trend confirmation dimension.
pub const TREND_CONFIRMATION_MAX: i32 = 25;
/// Maximum score for fundamental confirmation dimension.
pub const FUNDAMENTAL_CONFIRMATION_MAX: i32 = 25;
/// Maximum score for catalyst quality dimension.
pub const CATALYST_QUALITY_MAX: i32 = 25;
/// Maximum score for historical transferability dimension.
pub const HISTORICAL_TRANSFERABILITY_MAX: i32 = 15;
/// Maximum score for cross-agent consistency dimension.
pub const CROSS_AGENT_CONSISTENCY_MAX: i32 = 25;
/// Maximum score for risk clarity dimension.
pub const RISK_CLARITY_MAX: i32 = 15;
/// Floor score when catalyst vacuum is detected (no news report).
pub const CATALYST_VACUUM_FLOOR: i32 = 3;

/// Final confidence score with breakdown and applied caps.
pub struct ConfidenceAssessment {
    pub final_score: i32,
    pub breakdown: ConfidenceBreakdown,
    pub profile: ConfidenceProfile,
    pub caps: Vec<ConfidenceCap>,
}

/// Final direction score with component breakdown.
pub struct DirectionAssessment {
    pub final_score: i32,
    pub breakdown: DirectionBreakdown,
}

/// Final action score with component breakdown.
pub struct ActionAssessment {
    pub final_score: i32,
    pub breakdown: ActionBreakdown,
}

/// Calibrated recommendation — rating, action, and narrative text.
pub struct RecommendationCalibration {
    pub final_rating: String,
    pub final_action: String,
    pub rationale: LocalText,
    pub decision_narrative: LocalText,
}

/// Thresholds for calibrating recommendations based on historical performance.
#[derive(Clone, Debug)]
pub struct CalibrationProfile {
    pub min_confidence_score: i32,
    pub min_action_score: i32,
    pub direction_floor_abs: i32,
    pub strong_direction_abs: i32,
    pub sample_count: usize,
    pub min_hit_rate: f64,
    pub min_avg_alpha_return: f64,
}

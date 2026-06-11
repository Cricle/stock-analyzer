const DATA_QUALITY_MAX: i32 = 20;
const TREND_CONFIRMATION_MAX: i32 = 20;
const FUNDAMENTAL_CONFIRMATION_MAX: i32 = 20;
const CATALYST_QUALITY_MAX: i32 = 15;
const HISTORICAL_TRANSFERABILITY_MAX: i32 = 10;
const CROSS_AGENT_CONSISTENCY_MAX: i32 = 15;
const RISK_CLARITY_MAX: i32 = 10;

pub struct ConfidenceAssessment {
    pub final_score: i32,
    pub breakdown: ConfidenceBreakdown,
    pub profile: ConfidenceProfile,
    pub caps: Vec<ConfidenceCap>,
}

pub struct DirectionAssessment {
    pub final_score: i32,
    pub breakdown: DirectionBreakdown,
}

pub struct ActionAssessment {
    pub final_score: i32,
    pub breakdown: ActionBreakdown,
}

pub struct RecommendationCalibration {
    pub final_rating: String,
    pub final_action: String,
    pub rationale: LocalText,
    pub decision_narrative: LocalText,
}

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

impl Default for CalibrationProfile {
    fn default() -> Self {
        Self {
            min_confidence_score: 45,
            min_action_score: 35,
            direction_floor_abs: 12,
            strong_direction_abs: 35,
            sample_count: 0,
            min_hit_rate: 0.0,
            min_avg_alpha_return: 0.0,
        }
    }
}

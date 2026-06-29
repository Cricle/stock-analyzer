impl Default for CalibrationProfile {
    fn default() -> Self {
        Self {
            min_confidence_score: 45,
            min_action_score: 35,
            direction_floor_abs: 8,
            strong_direction_abs: 50,
            sample_count: 0,
            min_hit_rate: 0.0,
            min_avg_alpha_return: 0.0,
        }
    }
}

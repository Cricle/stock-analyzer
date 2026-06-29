impl Default for CalibrationProfile {
    fn default() -> Self {
        Self {
            min_confidence_score: 45,
            min_action_score: 35,
            direction_floor_abs: 8,
            // Lowered from 50: when the LLM defaults to Hold, rating_bias=0
            // so the direction signal comes purely from analyst probabilities.
            // Max possible without rating contribution: 25+25+20+15+15 = 100,
            // but typical moderate-bearish is ~25-30.
            strong_direction_abs: 20,
            sample_count: 0,
            min_hit_rate: 0.0,
            min_avg_alpha_return: 0.0,
        }
    }
}


pub fn history_requires_caution(profile: &CalibrationProfile) -> bool {
    profile.sample_count >= 12
        && (profile.min_hit_rate < 0.5 || profile.min_avg_alpha_return <= 0.0)
}

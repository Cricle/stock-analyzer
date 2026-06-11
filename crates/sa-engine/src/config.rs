//! Configuration helpers ported from the backend.

/// Parse an env var as a boolean flag.
pub fn env_flag_value(v: &str) -> bool {
    matches!(v, "1" | "true" | "TRUE" | "yes" | "YES")
}

/// Check an env var as a boolean flag with a default.
pub fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| env_flag_value(&v))
        .unwrap_or(false)
}

/// Whether ANALYSIS_DEBUG_MODE or ANALYSIS_DEBUG_QUICK_ONLY is set.
pub fn analysis_debug_quick_only() -> bool {
    env_flag("ANALYSIS_DEBUG_MODE") || env_flag("ANALYSIS_DEBUG_QUICK_ONLY")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_flag_value_true_cases() {
        assert!(env_flag_value("1"));
        assert!(env_flag_value("true"));
        assert!(env_flag_value("TRUE"));
        assert!(env_flag_value("yes"));
        assert!(env_flag_value("YES"));
    }

    #[test]
    fn env_flag_value_false_cases() {
        assert!(!env_flag_value("0"));
        assert!(!env_flag_value("false"));
        assert!(!env_flag_value("no"));
        assert!(!env_flag_value(""));
        assert!(!env_flag_value("random"));
    }

    #[test]
    fn env_flag_missing_var_returns_false() {
        assert!(!env_flag("NONEXISTENT_VAR_XYZ_12345"));
    }

    #[test]
    fn env_flag_existing_var() {
        std::env::set_var("TEST_ENV_FLAG_XYZ", "true");
        assert!(env_flag("TEST_ENV_FLAG_XYZ"));
        std::env::remove_var("TEST_ENV_FLAG_XYZ");
    }
}

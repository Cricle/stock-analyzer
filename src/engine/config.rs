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
    fn env_flag_value_true_variants() {
        for v in &["1", "true", "TRUE", "yes", "YES"] {
            assert!(env_flag_value(v), "expected true for {:?}", v);
        }
    }

    #[test]
    fn env_flag_value_false_variants() {
        for v in &["0", "false", "FALSE", "no", "NO", "", "True", "Yes", "y"] {
            assert!(!env_flag_value(v), "expected false for {:?}", v);
        }
    }

    #[test]
    fn env_flag_missing_returns_false() {
        assert!(!env_flag("DEFINITELY_NOT_SET_VAR_12345"));
    }
}

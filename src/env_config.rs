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

/// Number of bull/bear debate rounds (default: 3).
pub fn debate_rounds() -> usize {
    std::env::var("DEBATE_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
}

/// Number of risk discussion rounds (default: 2).
pub fn risk_discuss_rounds() -> usize {
    std::env::var("RISK_DISCUSS_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2)
}

/// Whether SEC fundamentals fallback is enabled (default: true).
pub fn fundamentals_fallback_enabled() -> bool {
    std::env::var("FUNDAMENTALS_FALLBACK_ENABLED")
        .map(|v| env_flag_value(&v))
        .unwrap_or(true)
}

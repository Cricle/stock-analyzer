use crate::scoring::score_types::ScoreWeights;

/// Score configuration — loaded from config file + environment variables.
///
/// # Priority (highest wins)
/// 1. Environment variables (`SCORE_WEIGHT_*`, `SCORE_SENTIMENT_NEWS_LIMIT`)
/// 2. `[scoring]` section in `config.toml`
/// 3. Default values
#[derive(Debug, Clone)]
pub struct ScoreConfig {
    pub weights: ScoreWeights,
    /// Max number of news headlines to send to LLM for sentiment.
    pub sentiment_news_limit: usize,
    /// Confidence cap thresholds — upper-bound limits applied when
    /// data quality, history, or execution conditions are weak.
    pub caps: ConfidenceCapsConfig,
}

/// Confidence cap configuration. Each field caps the maximum confidence
/// score when the corresponding condition is triggered.
///
/// Set to 100 to effectively disable a cap.
#[derive(Debug, Clone)]
pub struct ConfidenceCapsConfig {
    /// Cap when core reports < 3 or tool failures >= 2
    pub missing_core_data: i32,
    /// Cap when evidence density < 1.5
    pub thin_evidence_density: i32,
    /// Cap when execution boundary is incomplete
    pub execution_boundary_missing: i32,
    /// Cap when cross-agent consistency score <= 8
    pub cross_agent_divergence: i32,
    /// Cap when history exists but is thin (some matches)
    pub thin_setup_history_with_data: i32,
    /// Cap when history is completely absent
    pub thin_setup_history_no_data: i32,
    /// Cap when next_steps count == 0
    pub missing_follow_up_plan: i32,
    /// Cap when blocking evidence gaps exist
    pub decision_blocking_gaps_present: i32,
    /// Cap when fundamentals period is mixed
    pub fundamentals_period_mixed: i32,
    /// Cap near resistance without fresh catalyst
    pub near_resistance_without_fresh_catalyst: i32,
    /// Cap when zero resolved setup history
    pub zero_resolved_setup_history: i32,
}

impl Default for ConfidenceCapsConfig {
    fn default() -> Self {
        Self {
            missing_core_data: 80,
            thin_evidence_density: 82,
            execution_boundary_missing: 83,
            cross_agent_divergence: 85,
            thin_setup_history_with_data: 85,
            thin_setup_history_no_data: 80,
            missing_follow_up_plan: 82,
            decision_blocking_gaps_present: 82,
            fundamentals_period_mixed: 80,
            near_resistance_without_fresh_catalyst: 80,
            zero_resolved_setup_history: 82,
        }
    }
}

impl Default for ScoreConfig {
    fn default() -> Self {
        Self {
            weights: ScoreWeights::default(),
            sentiment_news_limit: 10,
            caps: ConfidenceCapsConfig::default(),
        }
    }
}

impl ScoreConfig {
    /// Apply environment variable overrides on top of current values.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(w) = std::env::var("SCORE_WEIGHT_TECHNICAL")
            && let Ok(v) = w.parse()
        {
            self.weights.technical = v;
        }
        if let Ok(w) = std::env::var("SCORE_WEIGHT_FUNDAMENTAL")
            && let Ok(v) = w.parse()
        {
            self.weights.fundamental = v;
        }
        if let Ok(w) = std::env::var("SCORE_WEIGHT_SENTIMENT")
            && let Ok(v) = w.parse()
        {
            self.weights.sentiment = v;
        }
        if let Ok(w) = std::env::var("SCORE_WEIGHT_LLM_ANALYSIS")
            && let Ok(v) = w.parse()
        {
            self.weights.llm_analysis = v;
        }
        if let Ok(n) = std::env::var("SCORE_SENTIMENT_NEWS_LIMIT")
            && let Ok(v) = n.parse()
        {
            self.sentiment_news_limit = v;
        }
        self.caps.apply_env_overrides();
    }

    /// Load from environment variables only (backward-compatible).
    pub fn from_env() -> Self {
        let mut config = Self::default();
        config.apply_env_overrides();
        config
    }
}

impl ConfidenceCapsConfig {
    /// Apply env var overrides: `CONF_CAP_<KEY>=<value>` (e.g. `CONF_CAP_MISSING_CORE_DATA=90`).
    pub fn apply_env_overrides(&mut self) {
        macro_rules! cap_env {
            ($field:ident, $name:literal) => {
                if let Ok(v) = std::env::var(concat!("CONF_CAP_", $name))
                    && let Ok(n) = v.parse()
                {
                    self.$field = n;
                }
            };
        }
        cap_env!(missing_core_data, "MISSING_CORE_DATA");
        cap_env!(thin_evidence_density, "THIN_EVIDENCE_DENSITY");
        cap_env!(execution_boundary_missing, "EXECUTION_BOUNDARY_MISSING");
        cap_env!(cross_agent_divergence, "CROSS_AGENT_DIVERGENCE");
        cap_env!(thin_setup_history_with_data, "THIN_SETUP_HISTORY_WITH_DATA");
        cap_env!(thin_setup_history_no_data, "THIN_SETUP_HISTORY_NO_DATA");
        cap_env!(missing_follow_up_plan, "MISSING_FOLLOW_UP_PLAN");
        cap_env!(decision_blocking_gaps_present, "DECISION_BLOCKING_GAPS_PRESENT");
        cap_env!(fundamentals_period_mixed, "FUNDAMENTALS_PERIOD_MIXED");
        cap_env!(near_resistance_without_fresh_catalyst, "NEAR_RESISTANCE_WITHOUT_FRESH_CATALYST");
        cap_env!(zero_resolved_setup_history, "ZERO_RESOLVED_SETUP_HISTORY");
    }
}

//! Configuration loader — reads `~/.config/sa-engine/config.toml`
//! (or path from `SA_ENGINE_CONFIG` env var), then merges environment
//! variable overrides.
//!
//! # Priority (highest wins)
//! 1. Environment variables
//! 2. config.toml file
//! 3. Default values

use serde::Deserialize;

use crate::scoring::config::ScoreConfig;

/// Top-level config matching `config.toml` structure.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SaConfig {
    #[serde(default)]
    pub scoring: Option<ScoringSection>,
    #[serde(default)]
    pub api_keys: Option<ApiKeysSection>,
    #[serde(default)]
    pub confidence_caps: Option<ConfidenceCapsSection>,
}

/// Scoring configuration — dimension weights and sentiment limits.
#[derive(Debug, Clone, Deserialize)]
pub struct ScoringSection {
    #[serde(default)]
    pub weights: Option<ScoringWeightsSection>,
    #[serde(default)]
    pub sentiment_news_limit: Option<usize>,
}

/// Per-dimension weight overrides (0–100).
#[derive(Debug, Clone, Deserialize)]
pub struct ScoringWeightsSection {
    pub technical: Option<u8>,
    pub fundamental: Option<u8>,
    pub sentiment: Option<u8>,
    pub llm_analysis: Option<u8>,
}

/// Third-party API keys used by data providers.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiKeysSection {
    #[serde(default)]
    pub finnhub: Vec<String>,
}

/// Per-scenario confidence score caps — penalize thin evidence or missing data.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfidenceCapsSection {
    pub missing_core_data: Option<i32>,
    pub thin_evidence_density: Option<i32>,
    pub execution_boundary_missing: Option<i32>,
    pub cross_agent_divergence: Option<i32>,
    pub thin_setup_history_with_data: Option<i32>,
    pub thin_setup_history_no_data: Option<i32>,
    pub missing_follow_up_plan: Option<i32>,
    pub decision_blocking_gaps_present: Option<i32>,
    pub fundamentals_period_mixed: Option<i32>,
    pub near_resistance_without_fresh_catalyst: Option<i32>,
    pub zero_resolved_setup_history: Option<i32>,
}

impl SaConfig {
    /// Resolve the config file path:
    /// 1. `SA_ENGINE_CONFIG` env var, or
    /// 2. `~/.config/sa-engine/config.toml`
    pub fn config_path() -> Option<std::path::PathBuf> {
        if let Ok(path) = std::env::var("SA_ENGINE_CONFIG") {
            let p = std::path::PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
        let home = dirs::home_dir()?;
        let default = home.join(".config").join("sa-engine").join("config.toml");
        default.exists().then_some(default)
    }

    /// Load config from file, falling back to defaults.
    pub fn load() -> Self {
        let path = Self::config_path();
        match path {
            Some(ref p) => match std::fs::read_to_string(p) {
                Ok(content) => match toml::from_str::<SaConfig>(&content) {
                    Ok(cfg) => {
                        tracing::debug!(path = %p.display(), "loaded config file");
                        cfg
                    }
                    Err(e) => {
                        tracing::warn!(path = %p.display(), error = %e, "config file parse error, using defaults");
                        SaConfig::default()
                    }
                },
                Err(e) => {
                    tracing::warn!(path = %p.display(), error = %e, "cannot read config file, using defaults");
                    SaConfig::default()
                }
            },
            None => SaConfig::default(),
        }
    }

    /// Build `ScoreConfig` from file config + env var overrides.
    ///
    /// Priority: env var > config.toml > default
    pub fn score_config(&self) -> ScoreConfig {
        let mut cfg = ScoreConfig::default();

        // Layer 1: config.toml [scoring.weights]
        if let Some(ref scoring) = self.scoring {
            if let Some(ref w) = scoring.weights {
                if let Some(v) = w.technical {
                    cfg.weights.technical = v;
                }
                if let Some(v) = w.fundamental {
                    cfg.weights.fundamental = v;
                }
                if let Some(v) = w.sentiment {
                    cfg.weights.sentiment = v;
                }
                if let Some(v) = w.llm_analysis {
                    cfg.weights.llm_analysis = v;
                }
            }
            if let Some(v) = scoring.sentiment_news_limit {
                cfg.sentiment_news_limit = v;
            }
        }

        // Layer 2: env var overrides
        cfg.apply_env_overrides();

        // Layer 1b: config.toml [confidence_caps]
        if let Some(ref caps) = self.confidence_caps {
            macro_rules! cap_field {
                ($field:ident) => {
                    if let Some(v) = caps.$field {
                        cfg.caps.$field = v;
                    }
                };
            }
            cap_field!(missing_core_data);
            cap_field!(thin_evidence_density);
            cap_field!(execution_boundary_missing);
            cap_field!(cross_agent_divergence);
            cap_field!(thin_setup_history_with_data);
            cap_field!(thin_setup_history_no_data);
            cap_field!(missing_follow_up_plan);
            cap_field!(decision_blocking_gaps_present);
            cap_field!(fundamentals_period_mixed);
            cap_field!(near_resistance_without_fresh_catalyst);
            cap_field!(zero_resolved_setup_history);
        }

        // Layer 2: env var overrides (also covers caps via CONF_CAP_*)
        cfg.apply_env_overrides();

        cfg
    }
}

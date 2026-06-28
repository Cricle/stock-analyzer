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
}

impl Default for ScoreConfig {
    fn default() -> Self {
        Self {
            weights: ScoreWeights::default(),
            sentiment_news_limit: 10,
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
    }

    /// Load from environment variables only (backward-compatible).
    pub fn from_env() -> Self {
        let mut config = Self::default();
        config.apply_env_overrides();
        config
    }
}

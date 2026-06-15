use crate::engine::score::types::ScoreWeights;

/// Score configuration loaded from environment.
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
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(w) = std::env::var("SCORE_WEIGHT_TECHNICAL")
            && let Ok(v) = w.parse() {
                config.weights.technical = v;
            }
        if let Ok(w) = std::env::var("SCORE_WEIGHT_FUNDAMENTAL")
            && let Ok(v) = w.parse() {
                config.weights.fundamental = v;
            }
        if let Ok(w) = std::env::var("SCORE_WEIGHT_SENTIMENT")
            && let Ok(v) = w.parse() {
                config.weights.sentiment = v;
            }
        if let Ok(w) = std::env::var("SCORE_WEIGHT_LLM_ANALYSIS")
            && let Ok(v) = w.parse() {
                config.weights.llm_analysis = v;
            }
        if let Ok(n) = std::env::var("SCORE_SENTIMENT_NEWS_LIMIT")
            && let Ok(v) = n.parse() {
                config.sentiment_news_limit = v;
            }
        config
    }
}

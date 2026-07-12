use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Indicates how much trust to place in a dimension score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ScoreReliability {
    /// All required data present, score computed from real signals.
    #[default]
    High,
    /// Some data missing or degraded; score is a rough estimate.
    Low,
    /// Required data entirely missing; score is a hardcoded fallback.
    Missing,
}


impl std::fmt::Display for ScoreReliability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::High => write!(f, "high"),
            Self::Low => write!(f, "low"),
            Self::Missing => write!(f, "missing"),
        }
    }
}

/// Per-dimension score with reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub score: u8,
    pub reason: String,
    #[serde(default)]
    pub reliability: ScoreReliability,
}

/// Full score for a single stock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockScore {
    pub symbol: String,
    pub market: String,
    pub total: u8,
    pub technical: DimensionScore,
    pub fundamental: DimensionScore,
    pub sentiment: DimensionScore,
    pub llm_analysis: DimensionScore,
    pub scored_at: DateTime<Utc>,
}

/// Configurable weights for each dimension. Must sum to 100.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreWeights {
    pub technical: u8,
    pub fundamental: u8,
    pub sentiment: u8,
    pub llm_analysis: u8,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            technical: 30,
            fundamental: 25,
            sentiment: 20,
            llm_analysis: 25,
        }
    }
}

impl ScoreWeights {
    pub fn validate(&self) -> anyhow::Result<()> {
        let sum = self.technical as u16
            + self.fundamental as u16
            + self.sentiment as u16
            + self.llm_analysis as u16;
        anyhow::ensure!(sum == 100, "weights must sum to 100, got {sum}");
        Ok(())
    }
}

/// Score threshold labels.
pub fn score_label(score: u8) -> &'static str {
    match score {
        80..=100 => "strong_buy",
        65..=79 => "buy",
        45..=64 => "neutral",
        30..=44 => "cautious",
        _ => "avoid",
    }
}

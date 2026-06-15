use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Per-dimension score with reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub score: u8,
    pub reason: String,
    /// I18n key(s) for the reason, joined by the same separator as `reason`.
    /// The rendering layer can resolve each key to produce a localized reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_key: Option<String>,
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

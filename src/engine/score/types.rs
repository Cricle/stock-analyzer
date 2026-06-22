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

#[cfg(test)]
mod score_types_tests {
    use super::*;

    // --- ScoreWeights::validate ---

    #[test]
    fn score_weights_validate_valid() {
        let weights = ScoreWeights::default();
        assert!(weights.validate().is_ok());
    }

    #[test]
    fn score_weights_validate_custom() {
        let weights = ScoreWeights {
            technical: 40,
            fundamental: 30,
            sentiment: 15,
            llm_analysis: 15,
        };
        assert!(weights.validate().is_ok());
    }

    #[test]
    fn score_weights_validate_invalid() {
        let weights = ScoreWeights {
            technical: 50,
            fundamental: 50,
            sentiment: 50,
            llm_analysis: 50,
        };
        assert!(weights.validate().is_err());
    }

    #[test]
    fn score_weights_validate_zero() {
        let weights = ScoreWeights {
            technical: 0,
            fundamental: 0,
            sentiment: 0,
            llm_analysis: 0,
        };
        assert!(weights.validate().is_err());
    }

    // --- score_label ---

    #[test]
    fn score_label_strong_buy() {
        assert_eq!(score_label(80), "strong_buy");
        assert_eq!(score_label(100), "strong_buy");
        assert_eq!(score_label(90), "strong_buy");
    }

    #[test]
    fn score_label_buy() {
        assert_eq!(score_label(65), "buy");
        assert_eq!(score_label(79), "buy");
        assert_eq!(score_label(70), "buy");
    }

    #[test]
    fn score_label_neutral() {
        assert_eq!(score_label(45), "neutral");
        assert_eq!(score_label(64), "neutral");
        assert_eq!(score_label(55), "neutral");
    }

    #[test]
    fn score_label_cautious() {
        assert_eq!(score_label(30), "cautious");
        assert_eq!(score_label(44), "cautious");
        assert_eq!(score_label(35), "cautious");
    }

    #[test]
    fn score_label_avoid() {
        assert_eq!(score_label(0), "avoid");
        assert_eq!(score_label(29), "avoid");
        assert_eq!(score_label(15), "avoid");
    }
}

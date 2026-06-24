use crate::engine::score::config::ScoreConfig;
use crate::engine::score::dimensions::{
    fundamental::{self, FundamentalInput},
    llm_analysis::{self, LlmAnalysisInput},
    sentiment,
    technical::{self, TechnicalInput},
};
use crate::engine::score::score_types::{ScoreWeights, StockScore};
use chrono::Utc;

/// Minimal stock pick data needed for scoring.
/// Extracted from ta-engine's StockPickItem to avoid tight coupling.
pub struct ScoreablePick {
    pub symbol: String,
    pub market: String,
    // Technical
    pub rsi: Option<f64>,
    pub macd: Option<f64>,
    pub macd_signal: Option<f64>,
    pub macd_hist: Option<f64>,
    pub adx: Option<f64>,
    pub close_10_ema: Option<f64>,
    pub close_50_sma: Option<f64>,
    pub close_200_sma: Option<f64>,
    pub obv: Option<f64>,
    pub current_price: Option<f64>,
    pub volume_elevated: bool,
    pub latest_positive: bool,
    // Fundamental
    pub pe_like: Option<f64>,
    pub ps_like: Option<f64>,
    pub roe: Option<f64>,
    pub leverage: Option<f64>,
    pub market_cap: Option<f64>,
    pub revenues_usd: Option<f64>,
    pub net_income_usd: Option<f64>,
    // News / Sentiment
    pub news_headlines: Vec<String>,
    // LLM analysis cross-validation signals
    pub confidence: f64,
    pub objective_final_score: f64,
    pub momentum_score: f64,
    pub hit_rate: Option<f64>,
    pub catalyst_count: usize,
    pub hard_negative_count: usize,
    pub volume_ratio: Option<f64>,
    pub period_return_pct: Option<f64>,
}

/// Score a single stock pick across all 4 dimensions.
pub async fn score_stock_pick(
    llm: &crate::engine::llm::LlmClient,
    pick: &ScoreablePick,
    config: &ScoreConfig,
) -> StockScore {
    let technical_input = TechnicalInput {
        rsi: pick.rsi,
        macd: pick.macd,
        macd_signal: pick.macd_signal,
        macd_hist: pick.macd_hist,
        adx: pick.adx,
        close_10_ema: pick.close_10_ema,
        close_50_sma: pick.close_50_sma,
        close_200_sma: pick.close_200_sma,
        obv: pick.obv,
        current_price: pick.current_price,
        volume_elevated: pick.volume_elevated,
        latest_positive: pick.latest_positive,
    };
    let technical = technical::score_technical(&technical_input);

    let fundamental_input = FundamentalInput {
        pe_like: pick.pe_like,
        ps_like: pick.ps_like,
        roe: pick.roe,
        leverage: pick.leverage,
        market_cap: pick.market_cap,
        revenues_usd: pick.revenues_usd,
        net_income_usd: pick.net_income_usd,
    };
    let fundamental = fundamental::score_fundamental(&fundamental_input);

    let sentiment_score = sentiment::score_sentiment(
        llm,
        &pick.symbol,
        &pick.news_headlines,
        config.sentiment_news_limit,
    )
    .await;

    let llm_input = LlmAnalysisInput {
        confidence: pick.confidence,
        objective_final_score: pick.objective_final_score,
        momentum_score: pick.momentum_score,
        hit_rate: pick.hit_rate,
        catalyst_count: pick.catalyst_count,
        hard_negative_count: pick.hard_negative_count,
        volume_ratio: pick.volume_ratio,
        period_return_pct: pick.period_return_pct,
    };
    let llm_analysis = llm_analysis::score_llm_analysis(&llm_input);

    let total = weighted_total(
        &config.weights,
        &technical,
        &fundamental,
        &sentiment_score,
        &llm_analysis,
    );

    StockScore {
        symbol: pick.symbol.clone(),
        market: pick.market.clone(),
        total,
        technical,
        fundamental,
        sentiment: sentiment_score,
        llm_analysis,
        scored_at: Utc::now(),
    }
}

fn weighted_total(
    w: &ScoreWeights,
    technical: &crate::engine::score::score_types::DimensionScore,
    fundamental: &crate::engine::score::score_types::DimensionScore,
    sentiment: &crate::engine::score::score_types::DimensionScore,
    llm_analysis: &crate::engine::score::score_types::DimensionScore,
) -> u8 {
    let total = technical.score as f64 * w.technical as f64
        + fundamental.score as f64 * w.fundamental as f64
        + sentiment.score as f64 * w.sentiment as f64
        + llm_analysis.score as f64 * w.llm_analysis as f64;
    (total / 100.0).clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::score::score_types::DimensionScore;

    #[test]
    fn test_weighted_total_equal_weights() {
        let w = ScoreWeights {
            technical: 25,
            fundamental: 25,
            sentiment: 25,
            llm_analysis: 25,
        };
        let tech = DimensionScore {
            score: 80,
            reason: String::new(),
        };
        let fund = DimensionScore {
            score: 60,
            reason: String::new(),
        };
        let sent = DimensionScore {
            score: 40,
            reason: String::new(),
        };
        let llm = DimensionScore {
            score: 70,
            reason: String::new(),
        };
        let total = weighted_total(&w, &tech, &fund, &sent, &llm);
        assert_eq!(total, 62); // (80*25 + 60*25 + 40*25 + 70*25) / 100 = 62.5 -> 62
    }

    #[test]
    fn test_weighted_total_unequal_weights() {
        let w = ScoreWeights {
            technical: 50,
            fundamental: 20,
            sentiment: 15,
            llm_analysis: 15,
        };
        let tech = DimensionScore {
            score: 90,
            reason: String::new(),
        };
        let fund = DimensionScore {
            score: 30,
            reason: String::new(),
        };
        let sent = DimensionScore {
            score: 30,
            reason: String::new(),
        };
        let llm = DimensionScore {
            score: 30,
            reason: String::new(),
        };
        let total = weighted_total(&w, &tech, &fund, &sent, &llm);
        assert_eq!(total, 60);
    }

    #[test]
    fn test_weighted_total_all_max() {
        let w = ScoreWeights::default();
        let d = DimensionScore {
            score: 100,
            reason: String::new(),
        };
        let total = weighted_total(&w, &d, &d, &d, &d);
        assert_eq!(total, 100);
    }

    #[test]
    fn test_weighted_total_all_min() {
        let w = ScoreWeights::default();
        let d = DimensionScore {
            score: 0,
            reason: String::new(),
        };
        let total = weighted_total(&w, &d, &d, &d, &d);
        assert_eq!(total, 0);
    }
}

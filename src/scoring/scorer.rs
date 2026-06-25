use crate::scoring::config::ScoreConfig;
use crate::scoring::dimensions::{
    fundamental::{self, FundamentalInput},
    llm_analysis::{self, LlmAnalysisInput},
    sentiment,
    technical::{self, TechnicalInput},
};
use crate::scoring::score_types::{ScoreWeights, StockScore};
use chrono::Utc;

/// Minimal stock pick data needed for scoring.
/// Extracted from ta-engine's StockPickItem to avoid tight coupling.
pub struct ScoreablePick {
    pub symbol: String,
    pub market: String,
    pub technical: TechnicalInput,
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
    llm: &crate::llm::LlmClient,
    pick: &ScoreablePick,
    config: &ScoreConfig,
) -> StockScore {
    let technical = technical::score_technical(&pick.technical);

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

pub fn weighted_total(
    w: &ScoreWeights,
    technical: &crate::scoring::score_types::DimensionScore,
    fundamental: &crate::scoring::score_types::DimensionScore,
    sentiment: &crate::scoring::score_types::DimensionScore,
    llm_analysis: &crate::scoring::score_types::DimensionScore,
) -> u8 {
    let total = technical.score as f64 * w.technical as f64
        + fundamental.score as f64 * w.fundamental as f64
        + sentiment.score as f64 * w.sentiment as f64
        + llm_analysis.score as f64 * w.llm_analysis as f64;
    (total / 100.0).clamp(0.0, 100.0) as u8
}

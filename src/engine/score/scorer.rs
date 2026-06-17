use chrono::Utc;
use crate::engine::score::config::ScoreConfig;
use crate::engine::score::types::{StockScore, ScoreWeights};
use crate::engine::score::dimensions::{
    technical::{self, TechnicalInput},
    fundamental::{self, FundamentalInput},
    sentiment,
    llm_analysis::{self, LlmAnalysisInput},
};

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
    // Sentiment
    pub news_headlines: Vec<String>,
    // LLM Analysis
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
/// Returns an error if sentiment scoring fails.
pub async fn score_stock_pick(
    llm: &crate::engine::llm::LlmClient,
    pick: &ScoreablePick,
    config: &ScoreConfig,
) -> anyhow::Result<StockScore> {
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
    .await?;

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

    let total = weighted_total(&config.weights, &technical, &fundamental, &sentiment_score, &llm_analysis);

    Ok(StockScore {
        symbol: pick.symbol.clone(),
        market: pick.market.clone(),
        total,
        technical,
        fundamental,
        sentiment: sentiment_score,
        llm_analysis,
        scored_at: Utc::now(),
    })
}

fn weighted_total(
    w: &ScoreWeights,
    technical: &crate::engine::score::types::DimensionScore,
    fundamental: &crate::engine::score::types::DimensionScore,
    sentiment: &crate::engine::score::types::DimensionScore,
    llm_analysis: &crate::engine::score::types::DimensionScore,
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
    use crate::engine::score::types::ScoreWeights;

    fn dummy_pick() -> ScoreablePick {
        ScoreablePick {
            symbol: "TEST".into(),
            market: "A-share".into(),
            rsi: Some(50.0),
            macd: Some(0.1),
            macd_signal: Some(0.05),
            macd_hist: Some(0.05),
            adx: Some(25.0),
            close_10_ema: Some(100.0),
            close_50_sma: Some(98.0),
            close_200_sma: Some(95.0),
            obv: None,
            current_price: Some(101.0),
            volume_elevated: false,
            latest_positive: true,
            pe_like: Some(15.0),
            ps_like: None,
            roe: Some(15.0),
            leverage: Some(1.0),
            market_cap: Some(1e10),
            revenues_usd: Some(1e9),
            net_income_usd: Some(1e8),
            news_headlines: vec![],
            confidence: 60.0,
            objective_final_score: 60.0,
            momentum_score: 50.0,
            hit_rate: Some(0.5),
            catalyst_count: 2,
            hard_negative_count: 0,
            volume_ratio: Some(1.0),
            period_return_pct: Some(2.0),
        }
    }

    #[tokio::test]
    async fn test_weighted_total_range() {
        let tech = crate::engine::score::types::DimensionScore { score: 80, reason: String::new(), reason_key: None };
        let fund = crate::engine::score::types::DimensionScore { score: 60, reason: String::new(), reason_key: None };
        let sent = crate::engine::score::types::DimensionScore { score: 40, reason: String::new(), reason_key: None };
        let llm = crate::engine::score::types::DimensionScore { score: 70, reason: String::new(), reason_key: None };
        let weights = ScoreWeights { technical: 25, fundamental: 25, sentiment: 25, llm_analysis: 25 };
        let total = weighted_total(&weights, &tech, &fund, &sent, &llm);
        assert_eq!(total, 62); // (80+60+40+70)/4 = 62.5, truncated to 62
    }

    #[tokio::test]
    async fn test_weighted_total_extremes() {
        let tech = crate::engine::score::types::DimensionScore { score: 100, reason: String::new(), reason_key: None };
        let fund = crate::engine::score::types::DimensionScore { score: 100, reason: String::new(), reason_key: None };
        let sent = crate::engine::score::types::DimensionScore { score: 100, reason: String::new(), reason_key: None };
        let llm = crate::engine::score::types::DimensionScore { score: 100, reason: String::new(), reason_key: None };
        let weights = ScoreWeights { technical: 50, fundamental: 20, sentiment: 15, llm_analysis: 15 };
        let total = weighted_total(&weights, &tech, &fund, &sent, &llm);
        assert_eq!(total, 100);
    }

    #[tokio::test]
    async fn test_weighted_total_zeros() {
        let d = crate::engine::score::types::DimensionScore { score: 0, reason: String::new(), reason_key: None };
        let weights = ScoreWeights { technical: 25, fundamental: 25, sentiment: 25, llm_analysis: 25 };
        let total = weighted_total(&weights, &d, &d, &d, &d);
        assert_eq!(total, 0);
    }
}

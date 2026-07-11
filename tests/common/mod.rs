#[allow(dead_code)]
pub mod eval;
#[allow(dead_code)]
pub mod stocks;

use serde_json::Value;

#[allow(dead_code)]
pub fn load_fixture(name: &str) -> Value {
    let path = format!("tests/fixtures/{}.json", name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to load fixture {path}: {e}"));
    serde_json::from_str(&content).unwrap()
}

#[allow(dead_code)]
pub fn sample_scoreable_pick() -> stock_analyzer::score::scorer::ScoreablePick {
    stock_analyzer::score::scorer::ScoreablePick {
        symbol: "AAPL".into(),
        market: "美股".into(),
        technical: stock_analyzer::scoring::dimensions::technical::TechnicalInput {
            rsi: Some(55.0),
            macd: Some(0.3),
            macd_signal: Some(0.2),
            macd_hist: Some(0.1),
            adx: Some(25.0),
            close_10_ema: Some(183.0),
            close_50_sma: Some(180.0),
            close_200_sma: Some(170.0),
            obv: None,
            current_price: Some(185.5),
            volume_elevated: true,
            latest_positive: true,
        },
        pe_like: Some(28.0),
        ps_like: Some(7.0),
        roe: Some(150.0),
        leverage: Some(1.5),
        market_cap: Some(2_800_000_000_000.0),
        revenues_usd: Some(394_000_000_000.0),
        net_income_usd: Some(100_000_000_000.0),
        news_headlines: vec![
            "Apple announces record Q2 earnings".into(),
            "iPhone sales exceed expectations".into(),
        ],
        confidence: 72.0,
        objective_final_score: 68.0,
        momentum_score: 65.0,
        hit_rate: Some(0.65),
        catalyst_count: 3,
        hard_negative_count: 0,
        volume_ratio: Some(1.3),
        period_return_pct: Some(5.0),
    }
}

mod common;

#[test]
fn e2e_fixture_loading() {
    let market = common::load_fixture("sample_market");
    assert_eq!(market["symbol"], "AAPL");
    assert_eq!(market["price"], 185.50);

    let news = common::load_fixture("sample_news");
    assert_eq!(news["headlines"].as_array().unwrap().len(), 3);

    let llm = common::load_fixture("sample_llm_response");
    assert_eq!(llm["score"], 72);
}

#[test]
fn e2e_score_consistency_bullish() {
    let pick = common::sample_scoreable_pick();
    // Verify pick has all bullish signals
    assert!(pick.technical.rsi.unwrap() < 70.0);
    assert!(pick.technical.volume_elevated);
    assert!(pick.technical.latest_positive);
    // Score should be above neutral
    let tech = sa::score::dimensions::technical::score_technical(&pick.technical);
    assert!(
        tech.score >= 60,
        "expected bullish tech score, got {}",
        tech.score
    );
    assert!(tech.score <= 100);
}

#[test]
fn e2e_score_consistency_bearish() {
    let tech_input = sa::score::dimensions::technical::TechnicalInput {
        rsi: Some(80.0),
        macd: Some(-0.5),
        macd_signal: Some(-0.2),
        macd_hist: Some(-0.3),
        adx: Some(30.0),
        close_10_ema: Some(90.0),
        close_50_sma: Some(95.0),
        close_200_sma: Some(100.0),
        obv: None,
        current_price: Some(85.0),
        volume_elevated: true,
        latest_positive: false,
    };
    let tech = sa::score::dimensions::technical::score_technical(&tech_input);
    assert!(
        tech.score <= 40,
        "expected bearish tech score, got {}",
        tech.score
    );
}

#[test]
fn e2e_score_fundamental_mixed() {
    let fund_input = sa::score::dimensions::fundamental::FundamentalInput {
        pe_like: Some(10.0),
        ps_like: None,
        roe: Some(-5.0),
        leverage: Some(0.8),
        market_cap: None,
        revenues_usd: Some(1_000_000_000.0),
        net_income_usd: Some(-100_000_000.0),
    };
    let fund = sa::score::dimensions::fundamental::score_fundamental(&fund_input);
    assert!(
        fund.score >= 20 && fund.score <= 80,
        "mixed signals should be mid-range, got {}",
        fund.score
    );
}

#[test]
fn e2e_score_llm_analysis_consensus() {
    let llm_input = sa::score::dimensions::llm_analysis::LlmAnalysisInput {
        confidence: 70.0,
        objective_final_score: 70.0,
        momentum_score: 65.0,
        hit_rate: Some(0.65),
        catalyst_count: 6,
        hard_negative_count: 0,
        volume_ratio: Some(1.2),
        period_return_pct: Some(3.0),
    };
    let result = sa::score::dimensions::llm_analysis::score_llm_analysis(&llm_input);
    assert!(
        result.score >= 55,
        "expected decent score with consensus, got {}",
        result.score
    );
}

#[test]
fn e2e_score_label_mapping() {
    assert_eq!(sa::score::types::score_label(85), "strong_buy");
    assert_eq!(sa::score::types::score_label(70), "buy");
    assert_eq!(sa::score::types::score_label(55), "neutral");
    assert_eq!(sa::score::types::score_label(35), "cautious");
    assert_eq!(sa::score::types::score_label(20), "avoid");
}

#[test]
fn e2e_score_weights_validation() {
    let weights = sa::score::types::ScoreWeights::default();
    assert!(weights.validate().is_ok());

    let invalid = sa::score::types::ScoreWeights {
        technical: 50,
        fundamental: 50,
        sentiment: 50,
        llm_analysis: 50,
    };
    assert!(invalid.validate().is_err());
}

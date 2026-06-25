use sa::scoring::DimensionScore;
use sa::scoring::dimensions::technical::{TechnicalInput, score_technical};

fn bullish_input() -> TechnicalInput {
    TechnicalInput {
        rsi: Some(25.0),
        macd: Some(0.5),
        macd_signal: Some(0.3),
        macd_hist: Some(0.2),
        adx: Some(30.0),
        close_10_ema: Some(100.0),
        close_50_sma: Some(95.0),
        close_200_sma: Some(90.0),
        obv: None,
        current_price: Some(105.0),
        volume_elevated: true,
        latest_positive: true,
    }
}

#[test]
fn test_oversold_rsi_bullish() {
    let result = score_technical(&bullish_input());
    assert!(
        result.score >= 70,
        "expected high score for oversold+bullish, got {}",
        result.score
    );
    assert!(result.reason.contains("超卖"));
}

#[test]
fn test_overbought_bearish() {
    let input = TechnicalInput {
        rsi: Some(80.0),
        macd: Some(0.1),
        macd_signal: Some(0.3),
        macd_hist: Some(-0.2),
        adx: Some(25.0),
        close_10_ema: Some(90.0),
        close_50_sma: Some(95.0),
        close_200_sma: Some(100.0),
        obv: None,
        current_price: Some(88.0),
        volume_elevated: true,
        latest_positive: false,
    };
    let result = score_technical(&input);
    assert!(
        result.score <= 40,
        "expected low score for overbought+bearish, got {}",
        result.score
    );
}

#[test]
fn test_no_data_returns_neutral() {
    let input = TechnicalInput {
        rsi: None,
        macd: None,
        macd_signal: None,
        macd_hist: None,
        adx: None,
        close_10_ema: None,
        close_50_sma: None,
        close_200_sma: None,
        obv: None,
        current_price: None,
        volume_elevated: false,
        latest_positive: false,
    };
    let result = score_technical(&input);
    assert!(
        result.score >= 40 && result.score <= 60,
        "expected neutral, got {}",
        result.score
    );
}

#[test]
fn test_partial_data_rsi_only() {
    let input = TechnicalInput {
        rsi: Some(45.0),
        macd: None,
        macd_signal: None,
        macd_hist: None,
        adx: None,
        close_10_ema: None,
        close_50_sma: None,
        close_200_sma: None,
        obv: None,
        current_price: None,
        volume_elevated: false,
        latest_positive: false,
    };
    let result = score_technical(&input);
    // RSI 45 is neutral zone, MACD/MA/volume get neutral defaults
    assert!(
        result.score >= 35 && result.score <= 65,
        "expected near-neutral with partial data, got {}",
        result.score
    );
}

#[test]
fn test_macd_death_cross() {
    let input = TechnicalInput {
        rsi: Some(50.0),
        macd: Some(-0.3),
        macd_signal: Some(-0.1),
        macd_hist: Some(-0.2),
        adx: None,
        close_10_ema: Some(100.0),
        close_50_sma: Some(100.0),
        close_200_sma: Some(100.0),
        obv: None,
        current_price: Some(100.0),
        volume_elevated: false,
        latest_positive: false,
    };
    let result = score_technical(&input);
    assert!(
        result.reason.contains("死叉"),
        "expected death cross reason, got {}",
        result.reason
    );
}

#[test]
fn test_ma_bearish_alignment() {
    let input = TechnicalInput {
        rsi: Some(50.0),
        macd: None,
        macd_signal: None,
        macd_hist: None,
        adx: None,
        close_10_ema: Some(90.0),
        close_50_sma: Some(95.0),
        close_200_sma: Some(100.0),
        obv: None,
        current_price: Some(85.0),
        volume_elevated: false,
        latest_positive: false,
    };
    let result = score_technical(&input);
    assert!(
        result.reason.contains("空头排列"),
        "expected bearish MA reason, got {}",
        result.reason
    );
}

#[test]
fn test_volume_elevated_negative() {
    let input = TechnicalInput {
        rsi: Some(50.0),
        macd: None,
        macd_signal: None,
        macd_hist: None,
        adx: None,
        close_10_ema: None,
        close_50_sma: None,
        close_200_sma: None,
        obv: None,
        current_price: None,
        volume_elevated: true,
        latest_positive: false,
    };
    let result = score_technical(&input);
    assert!(
        result.reason.contains("放量下跌"),
        "expected volume drop reason, got {}",
        result.reason
    );
    assert!(
        result.score < 50,
        "expected below neutral for volume drop, got {}",
        result.score
    );
}

#[test]
fn test_rsi_high_zone() {
    let input = TechnicalInput {
        rsi: Some(65.0),
        macd: None,
        macd_signal: None,
        macd_hist: None,
        adx: None,
        close_10_ema: None,
        close_50_sma: None,
        close_200_sma: None,
        obv: None,
        current_price: None,
        volume_elevated: false,
        latest_positive: false,
    };
    let result = score_technical(&input);
    assert!(
        result.reason.contains("偏高"),
        "expected high RSI reason, got {}",
        result.reason
    );
}

#[test]
fn test_macd_golden_cross() {
    let input = TechnicalInput {
        rsi: Some(50.0),
        macd: Some(0.5),
        macd_signal: Some(0.3),
        macd_hist: Some(0.2),
        adx: None,
        close_10_ema: Some(100.0),
        close_50_sma: Some(100.0),
        close_200_sma: Some(100.0),
        obv: None,
        current_price: Some(100.0),
        volume_elevated: false,
        latest_positive: false,
    };
    let result = score_technical(&input);
    assert!(
        result.reason.contains("金叉"),
        "expected golden cross reason, got {}",
        result.reason
    );
}

#[test]
fn test_ma_bullish_alignment() {
    let input = TechnicalInput {
        rsi: Some(50.0),
        macd: None,
        macd_signal: None,
        macd_hist: None,
        adx: None,
        close_10_ema: Some(110.0),
        close_50_sma: Some(105.0),
        close_200_sma: Some(100.0),
        obv: None,
        current_price: Some(115.0),
        volume_elevated: false,
        latest_positive: false,
    };
    let result = score_technical(&input);
    assert!(
        result.reason.contains("多头排列"),
        "expected bullish MA reason, got {}",
        result.reason
    );
}

#[test]
fn test_volume_elevated_positive() {
    let input = TechnicalInput {
        rsi: Some(50.0),
        macd: None,
        macd_signal: None,
        macd_hist: None,
        adx: None,
        close_10_ema: None,
        close_50_sma: None,
        close_200_sma: None,
        obv: None,
        current_price: None,
        volume_elevated: true,
        latest_positive: true,
    };
    let result = score_technical(&input);
    assert!(
        result.reason.contains("放量上涨"),
        "expected volume rise reason, got {}",
        result.reason
    );
    assert!(
        result.score > 50,
        "expected above neutral for volume rise, got {}",
        result.score
    );
}

#[test]
fn test_score_always_0_to_100() {
    // Test with extreme values
    let input = TechnicalInput {
        rsi: Some(100.0),
        macd: Some(-999.0),
        macd_signal: Some(999.0),
        macd_hist: Some(-999.0),
        adx: None,
        close_10_ema: Some(1.0),
        close_50_sma: Some(999.0),
        close_200_sma: Some(999.0),
        obv: None,
        current_price: Some(0.5),
        volume_elevated: true,
        latest_positive: false,
    };
    let result = score_technical(&input);
    assert!(
        result.score <= 100,
        "score should be <= 100, got {}",
        result.score
    );
}

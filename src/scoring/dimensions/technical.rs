use crate::scoring::score_types::DimensionScore;

/// Technical snapshot values from ta-engine's StockPickTechnicalSnapshot.
pub struct TechnicalInput {
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
}

pub fn score_technical(input: &TechnicalInput) -> DimensionScore {
    let mut total: f64 = 0.0;
    let mut weight_sum: f64 = 0.0;
    let mut reasons: Vec<String> = Vec::new();

    // RSI signal (weight 25)
    if let Some(rsi) = input.rsi {
        weight_sum += 25.0;
        if rsi < 30.0 {
            total += 25.0;
            reasons.push(format!("RSI {:.0} 超卖", rsi));
        } else if rsi < 40.0 {
            total += 18.0;
            reasons.push(format!("RSI {:.0} 偏低", rsi));
        } else if rsi <= 60.0 {
            total += 12.5;
        } else if rsi <= 70.0 {
            total += 8.0;
            reasons.push(format!("RSI {:.0} 偏高", rsi));
        } else {
            total += 0.0;
            reasons.push(format!("RSI {:.0} 超买", rsi));
        }
    }

    // MACD signal (weight 25)
    weight_sum += 25.0;
    let macd_bullish = match (input.macd, input.macd_signal, input.macd_hist) {
        (Some(macd), Some(sig), Some(hist)) => {
            if macd > sig && hist > 0.0 {
                reasons.push("MACD 金叉".into());
                true
            } else if macd < sig && hist < 0.0 {
                reasons.push("MACD 死叉".into());
                false
            } else {
                true // neutral
            }
        }
        _ => true, // no data, give neutral
    };
    total += if macd_bullish { 17.5 } else { 5.0 };

    // Moving average trend (weight 25)
    weight_sum += 25.0;
    let ma_score = if let Some(price) = input.current_price {
        let above_ema10 = input.close_10_ema.map(|e| price > e).unwrap_or(false);
        let above_sma50 = input.close_50_sma.map(|s| price > s).unwrap_or(false);
        let above_sma200 = input.close_200_sma.map(|s| price > s).unwrap_or(false);
        if above_ema10 && above_sma50 && above_sma200 {
            reasons.push("均线多头排列".into());
            25.0
        } else if above_sma50 && above_sma200 {
            20.0
        } else if above_sma200 {
            15.0
        } else if above_sma50 {
            10.0
        } else {
            reasons.push("均线空头排列".into());
            3.0
        }
    } else {
        12.5
    };
    total += ma_score;

    // Volume signal (weight 25)
    weight_sum += 25.0;
    let vol_score = if input.volume_elevated && input.latest_positive {
        reasons.push("放量上涨".into());
        25.0
    } else if input.volume_elevated && !input.latest_positive {
        reasons.push("放量下跌".into());
        5.0
    } else {
        12.5
    };
    total += vol_score;

    let score = if weight_sum > 0.0 {
        (total / weight_sum * 100.0).clamp(0.0, 100.0) as u8
    } else {
        50
    };

    DimensionScore {
        score,
        reason: if reasons.is_empty() {
            "技术面信号中性".into()
        } else {
            reasons.join("；")
        },
    }
}

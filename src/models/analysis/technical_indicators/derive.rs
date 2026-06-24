fn indicator_value(chart: &ReportMarketChart, key: &str) -> Option<f64> {
    chart
        .indicators
        .iter()
        .find(|item| item.key.eq_ignore_ascii_case(key))
        .and_then(|item| item.value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
}

fn derive_technical_indicators(chart: &ReportMarketChart) -> TechnicalIndicatorView {
    let current = chart.candles.last().map(|item| item.close);
    let values = TechnicalValues::from_chart(chart);
    let trend_items = vec![
        indicator_item("ma50", values.ma50, trend_signal(current, values.ma50), "trend", "main_chart", "lagging"),
        indicator_item("ma200", values.ma200, trend_signal(current, values.ma200), "long_trend", "main_chart", "lagging"),
        indicator_item("ema10", values.ema10, trend_signal(current, values.ema10), "short_trend", "main_chart", "lagging"),
        indicator_item("macd", values.macd, macd_signal(values.macd, values.macd_signal, values.macd_hist), "momentum_trend", "sub_chart", "lagging"),
        indicator_item("boll_mid", values.boll_mid, boll_signal(current, values.boll_upper, values.boll_lower), "volatility_channel", "main_chart", "lagging"),
        indicator_item("dmi_adx", values.adx, adx_signal(values.adx), "trend_strength", "sub_chart", "lagging"),
    ];
    let momentum_items = vec![
        indicator_item("rsi", values.rsi, rsi_signal(values.rsi), "overbought_oversold", "sub_chart", "leading"),
        indicator_item("kdj_k", values.kdj_k, kdj_signal(values.kdj_k, values.kdj_d, values.kdj_j), "short_reversal", "sub_chart", "leading"),
        indicator_item("cci", values.cci, cci_signal(values.cci), "deviation_reversal", "sub_chart", "leading"),
        indicator_item("wr", values.wr, wr_signal(values.wr), "overbought_oversold", "sub_chart", "leading"),
    ];
    let volatility_items = vec![
        indicator_item("atr", values.atr, atr_signal(values.atr, current), "stop_distance", "sub_chart", "lagging"),
        indicator_item("boll_width", values.boll_width, boll_width_signal(values.boll_width), "volatility_expansion", "main_chart", "lagging"),
    ];
    let volume_items = vec![
        indicator_item("obv", values.obv, obv_signal(values.obv_delta), "volume_confirmation", "sub_chart", "leading"),
        indicator_item("vwap", values.vwap, trend_signal(current, values.vwap), "institutional_cost", "main_chart", "lagging"),
        indicator_item("vwma", values.vwma, trend_signal(current, values.vwma), "volume_weighted_trend", "main_chart", "lagging"),
    ];
    let categories = vec![
        indicator_category("trend", "main_chart", "lagging", trend_items),
        indicator_category("momentum", "sub_chart", "leading", momentum_items),
        indicator_category("volatility", "sub_chart", "lagging", volatility_items),
        indicator_category("volume", "sub_chart", "leading", volume_items),
    ];
    let conclusions = derive_technical_conclusions(&values, current);
    TechnicalIndicatorView {
        categories,
        conclusions,
    }
}

#[derive(Default)]
pub(crate) struct TechnicalValues {
    ma50: Option<f64>,
    ma200: Option<f64>,
    ema10: Option<f64>,
    macd: Option<f64>,
    macd_signal: Option<f64>,
    macd_hist: Option<f64>,
    boll_mid: Option<f64>,
    boll_upper: Option<f64>,
    boll_lower: Option<f64>,
    boll_width: Option<f64>,
    adx: Option<f64>,
    rsi: Option<f64>,
    kdj_k: Option<f64>,
    kdj_d: Option<f64>,
    kdj_j: Option<f64>,
    cci: Option<f64>,
    wr: Option<f64>,
    atr: Option<f64>,
    obv: Option<f64>,
    obv_delta: Option<f64>,
    vwap: Option<f64>,
    vwma: Option<f64>,
}

impl TechnicalValues {
    fn from_chart(chart: &ReportMarketChart) -> Self {
        let candles = &chart.candles;
        let mut values = Self {
            ma50: indicator_value(chart, "close_50_sma").or_else(|| sma_report(candles, 50)),
            ma200: indicator_value(chart, "close_200_sma").or_else(|| sma_report(candles, 200)),
            ema10: indicator_value(chart, "close_10_ema").or_else(|| ema_report(candles, 10)),
            rsi: indicator_value(chart, "rsi").or_else(|| rsi_report(candles, 14)),
            atr: indicator_value(chart, "atr").or_else(|| atr_report(candles, 14)),
            vwma: indicator_value(chart, "vwma").or_else(|| vwma_report(candles, 20)),
            boll_mid: indicator_value(chart, "boll").or_else(|| sma_report(candles, 20)),
            boll_upper: indicator_value(chart, "boll_ub"),
            boll_lower: indicator_value(chart, "boll_lb"),
            macd: indicator_value(chart, "macd"),
            macd_signal: indicator_value(chart, "macds"),
            macd_hist: indicator_value(chart, "macdh"),
            ..Default::default()
        };
        if (values.boll_upper.is_none() || values.boll_lower.is_none())
            && let Some((mid, upper, lower)) = bollinger_report(candles, 20) {
                values.boll_mid = values.boll_mid.or(Some(mid));
                values.boll_upper = values.boll_upper.or(Some(upper));
                values.boll_lower = values.boll_lower.or(Some(lower));
            }
        values.boll_width = values
            .boll_upper
            .zip(values.boll_lower)
            .zip(values.boll_mid)
            .and_then(|((upper, lower), mid)| (mid.abs() > f64::EPSILON).then_some(((upper - lower) / mid) * 100.0));
        if (values.macd.is_none() || values.macd_signal.is_none() || values.macd_hist.is_none())
            && let Some((macd, signal, hist)) = macd_report(candles) {
                values.macd = values.macd.or(Some(macd));
                values.macd_signal = values.macd_signal.or(Some(signal));
                values.macd_hist = values.macd_hist.or(Some(hist));
            }
        if let Some((k, d, j)) = kdj_report(candles, 9) {
            values.kdj_k = Some(k);
            values.kdj_d = Some(d);
            values.kdj_j = Some(j);
        }
        values.cci = cci_report(candles, 20);
        values.wr = wr_report(candles, 14);
        values.adx = adx_report(candles, 14);
        if let Some((obv, delta)) = obv_report(candles) {
            values.obv = Some(obv);
            values.obv_delta = Some(delta);
        }
        values.vwap = vwap_report(candles, 20);
        values
    }
}

fn indicator_category(
    key: &str,
    display_mode: &str,
    signal_attribute: &str,
    indicators: Vec<TechnicalIndicatorItem>,
) -> TechnicalIndicatorCategory {
    TechnicalIndicatorCategory {
        key: key.to_string(),
        display_mode: display_mode.to_string(),
        signal_attribute: signal_attribute.to_string(),
        indicators,
    }
}

fn indicator_item(
    key: &str,
    value: Option<f64>,
    signal_code: &str,
    interpretation_code: &str,
    display_mode: &str,
    signal_attribute: &str,
) -> TechnicalIndicatorItem {
    TechnicalIndicatorItem {
        key: key.to_string(),
        value,
        signal_code: signal_code.to_string(),
        interpretation_code: interpretation_code.to_string(),
        display_mode: display_mode.to_string(),
        signal_attribute: signal_attribute.to_string(),
    }
}

pub(crate) fn derive_technical_conclusions(values: &TechnicalValues, current: Option<f64>) -> Vec<TechnicalIndicatorConclusion> {
    let mut out = Vec::new();
    if values.rsi.is_some_and(|value| value > 75.0) {
        out.push(technical_conclusion("technical_overheated", "warning", &["rsi"]));
    }
    if values.adx.is_some_and(|value| value >= 25.0)
        && values.macd_hist.is_some_and(|value| value < 0.0)
        && current
            .zip(values.ema10.or(values.ma50))
            .is_some_and(|(price, anchor)| price > anchor)
    {
        out.push(technical_conclusion(
            "trend_strength_with_fading_momentum",
            "warning",
            &["dmi_adx", "macd", "ema10"],
        ));
    }
    if values.macd.is_some_and(|value| value < 0.0) && current.zip(values.ema10).is_some_and(|(price, ema)| price > ema) {
        out.push(technical_conclusion("macd_momentum_lag", "warning", &["macd", "ema10"]));
    }
    if current.zip(values.ma50).is_some_and(|(price, ma)| price > ma)
        && values.ma50.zip(values.ma200).is_some_and(|(ma50, ma200)| ma50 > ma200)
    {
        out.push(technical_conclusion("trend_structure_positive", "success", &["ma50", "ma200"]));
    }
    if values.atr.zip(current).is_some_and(|(atr, price)| price > 0.0 && atr / price > 0.04) {
        out.push(technical_conclusion("volatility_elevated", "warning", &["atr"]));
    }
    if values.obv_delta.is_some_and(|delta| delta > 0.0) {
        out.push(technical_conclusion("volume_confirms_bid", "success", &["obv"]));
    }
    // RSI oversold
    if values.rsi.is_some_and(|value| value < 30.0) {
        out.push(technical_conclusion("technical_oversold", "success", &["rsi"]));
    }
    // RSI neutral zone (most common case)
    if values.rsi.is_some_and(|value| (30.0..=70.0).contains(&value)) {
        out.push(technical_conclusion("technical_neutral_zone", "neutral", &["rsi"]));
    }
    // MACD bullish (positive MACD and positive histogram)
    if values.macd.is_some_and(|value| value > 0.0) && values.macd_hist.is_some_and(|value| value > 0.0) {
        out.push(technical_conclusion("macd_bullish", "success", &["macd"]));
    }
    // MACD bearish (negative MACD and negative histogram)
    if values.macd.is_some_and(|value| value < 0.0) && values.macd_hist.is_some_and(|value| value < 0.0) {
        out.push(technical_conclusion("macd_bearish", "warning", &["macd"]));
    }
    // Price below MA50 (short-term weakness)
    if current.zip(values.ma50).is_some_and(|(price, ma)| price < ma) {
        out.push(technical_conclusion("price_below_ma50", "warning", &["ma50"]));
    }
    // Death cross (MA50 below MA200)
    if values.ma50.zip(values.ma200).is_some_and(|(ma50, ma200)| ma50 < ma200) {
        out.push(technical_conclusion("death_cross", "warning", &["ma50", "ma200"]));
    }
    // MACD/OBV cross-validation: MACD bullish but OBV bearish indicates volume divergence
    let macd_bullish = values
        .macd
        .zip(values.macd_signal)
        .zip(values.macd_hist)
        .is_some_and(|((macd, signal), hist)| macd > signal && hist > 0.0);
    let obv_bearish = values.obv_delta.is_some_and(|delta| delta < 0.0);
    if macd_bullish && obv_bearish {
        out.push(technical_conclusion(
            "macd_obv_divergence",
            "warning",
            &["macd", "obv"],
        ));
    }
    out
}

fn technical_conclusion(key: &str, severity: &str, evidence_keys: &[&str]) -> TechnicalIndicatorConclusion {
    TechnicalIndicatorConclusion {
        key: key.to_string(),
        severity: severity.to_string(),
        evidence_keys: evidence_keys.iter().map(|item| (*item).to_string()).collect(),
    }
}

fn trend_signal(current: Option<f64>, reference: Option<f64>) -> &'static str {
    match current.zip(reference) {
        Some((price, level)) if price > level => "above_reference",
        Some((price, level)) if price < level => "below_reference",
        Some(_) => "at_reference",
        None => "unavailable",
    }
}

fn macd_signal(macd: Option<f64>, signal: Option<f64>, hist: Option<f64>) -> &'static str {
    match (macd, signal, hist) {
        (Some(macd), Some(signal), Some(hist)) if macd > signal && hist > 0.0 => "bullish_cross",
        // MACD below signal with negative histogram, but MACD itself is above
        // zero — the bearish cross is happening in positive territory, meaning
        // downside momentum is weakening even though a death cross formed.
        (Some(macd), Some(signal), Some(hist)) if macd < signal && hist < 0.0 && macd > 0.0 => "weakening_bearish",
        (Some(macd), Some(signal), Some(hist)) if macd < signal && hist < 0.0 => "bearish_cross",
        (Some(macd), _, _) if macd > 0.0 => "above_zero",
        (Some(macd), _, _) if macd < 0.0 => "below_zero",
        _ => "unavailable",
    }
}

fn rsi_signal(value: Option<f64>) -> &'static str {
    match value {
        Some(value) if value > 70.0 => "overbought",
        Some(value) if value < 30.0 => "oversold",
        Some(_) => "neutral",
        None => "unavailable",
    }
}

fn kdj_signal(k: Option<f64>, d: Option<f64>, j: Option<f64>) -> &'static str {
    match (k, d, j) {
        (Some(k), Some(d), Some(j)) if k > 80.0 && d > 80.0 && j > 90.0 => "overbought",
        (Some(k), Some(d), Some(j)) if k < 20.0 && d < 20.0 && j < 10.0 => "oversold",
        (Some(k), Some(d), _) if k > d => "bullish_cross",
        (Some(k), Some(d), _) if k < d => "bearish_cross",
        _ => "unavailable",
    }
}

fn cci_signal(value: Option<f64>) -> &'static str {
    match value {
        Some(value) if value > 100.0 => "overbought",
        Some(value) if value < -100.0 => "oversold",
        Some(_) => "neutral",
        None => "unavailable",
    }
}

fn wr_signal(value: Option<f64>) -> &'static str {
    match value {
        Some(value) if value > -20.0 => "overbought",
        Some(value) if value < -80.0 => "oversold",
        Some(_) => "neutral",
        None => "unavailable",
    }
}

fn adx_signal(value: Option<f64>) -> &'static str {
    match value {
        Some(value) if value >= 25.0 => "trend_strong",
        Some(value) if value <= 20.0 => "range_bound",
        Some(_) => "trend_moderate",
        None => "unavailable",
    }
}

fn boll_signal(current: Option<f64>, upper: Option<f64>, lower: Option<f64>) -> &'static str {
    match (current, upper, lower) {
        (Some(price), Some(upper), _) if price >= upper => "upper_band_touch",
        (Some(price), _, Some(lower)) if price <= lower => "lower_band_touch",
        (Some(_), Some(_), Some(_)) => "inside_band",
        _ => "unavailable",
    }
}

fn atr_signal(atr: Option<f64>, current: Option<f64>) -> &'static str {
    match atr.zip(current) {
        Some((atr, price)) if price > 0.0 && atr / price > 0.04 => "high_volatility",
        Some((atr, price)) if price > 0.0 && atr / price < 0.015 => "low_volatility",
        Some(_) => "normal_volatility",
        None => "unavailable",
    }
}

fn boll_width_signal(value: Option<f64>) -> &'static str {
    match value {
        Some(value) if value > 12.0 => "band_expanding",
        Some(value) if value < 5.0 => "band_squeezing",
        Some(_) => "band_normal",
        None => "unavailable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- trend_signal ---

    #[test]
    fn trend_above() {
        assert_eq!(trend_signal(Some(110.0), Some(100.0)), "above_reference");
    }

    #[test]
    fn trend_below() {
        assert_eq!(trend_signal(Some(90.0), Some(100.0)), "below_reference");
    }

    #[test]
    fn trend_at() {
        assert_eq!(trend_signal(Some(100.0), Some(100.0)), "at_reference");
    }

    #[test]
    fn trend_none() {
        assert_eq!(trend_signal(None, Some(100.0)), "unavailable");
    }

    // --- macd_signal ---

    #[test]
    fn macd_bullish_cross() {
        assert_eq!(macd_signal(Some(0.5), Some(0.3), Some(0.2)), "bullish_cross");
    }

    #[test]
    fn macd_bearish_cross() {
        assert_eq!(macd_signal(Some(-0.5), Some(-0.3), Some(-0.2)), "bearish_cross");
    }

    #[test]
    fn macd_weakening_bearish() {
        assert_eq!(macd_signal(Some(0.1), Some(0.3), Some(-0.2)), "weakening_bearish");
    }

    #[test]
    fn macd_above_zero() {
        assert_eq!(macd_signal(Some(0.5), None, None), "above_zero");
    }

    #[test]
    fn macd_below_zero() {
        assert_eq!(macd_signal(Some(-0.5), None, None), "below_zero");
    }

    #[test]
    fn macd_unavailable() {
        assert_eq!(macd_signal(None, None, None), "unavailable");
    }

    // --- rsi_signal ---

    #[test]
    fn rsi_overbought() {
        assert_eq!(rsi_signal(Some(75.0)), "overbought");
    }

    #[test]
    fn rsi_oversold() {
        assert_eq!(rsi_signal(Some(25.0)), "oversold");
    }

    #[test]
    fn rsi_neutral() {
        assert_eq!(rsi_signal(Some(50.0)), "neutral");
    }

    #[test]
    fn rsi_unavailable() {
        assert_eq!(rsi_signal(None), "unavailable");
    }

    // --- kdj_signal ---

    #[test]
    fn kdj_overbought() {
        assert_eq!(kdj_signal(Some(85.0), Some(82.0), Some(95.0)), "overbought");
    }

    #[test]
    fn kdj_oversold() {
        assert_eq!(kdj_signal(Some(15.0), Some(18.0), Some(5.0)), "oversold");
    }

    #[test]
    fn kdj_bullish() {
        assert_eq!(kdj_signal(Some(60.0), Some(50.0), None), "bullish_cross");
    }

    #[test]
    fn kdj_bearish() {
        assert_eq!(kdj_signal(Some(40.0), Some(50.0), None), "bearish_cross");
    }

    #[test]
    fn kdj_unavailable() {
        assert_eq!(kdj_signal(None, None, None), "unavailable");
    }

    // --- cci_signal ---

    #[test]
    fn cci_overbought() {
        assert_eq!(cci_signal(Some(150.0)), "overbought");
    }

    #[test]
    fn cci_oversold() {
        assert_eq!(cci_signal(Some(-150.0)), "oversold");
    }

    #[test]
    fn cci_neutral() {
        assert_eq!(cci_signal(Some(0.0)), "neutral");
    }

    #[test]
    fn cci_unavailable() {
        assert_eq!(cci_signal(None), "unavailable");
    }

    // --- wr_signal ---

    #[test]
    fn wr_overbought() {
        assert_eq!(wr_signal(Some(-10.0)), "overbought");
    }

    #[test]
    fn wr_oversold() {
        assert_eq!(wr_signal(Some(-90.0)), "oversold");
    }

    #[test]
    fn wr_neutral() {
        assert_eq!(wr_signal(Some(-50.0)), "neutral");
    }

    #[test]
    fn wr_unavailable() {
        assert_eq!(wr_signal(None), "unavailable");
    }

    // --- adx_signal ---

    #[test]
    fn adx_strong() {
        assert_eq!(adx_signal(Some(30.0)), "trend_strong");
    }

    #[test]
    fn adx_range_bound() {
        assert_eq!(adx_signal(Some(15.0)), "range_bound");
    }

    #[test]
    fn adx_moderate() {
        assert_eq!(adx_signal(Some(22.0)), "trend_moderate");
    }

    #[test]
    fn adx_unavailable() {
        assert_eq!(adx_signal(None), "unavailable");
    }

    // --- boll_signal ---

    #[test]
    fn boll_upper_touch() {
        assert_eq!(boll_signal(Some(110.0), Some(110.0), Some(90.0)), "upper_band_touch");
    }

    #[test]
    fn boll_lower_touch() {
        assert_eq!(boll_signal(Some(90.0), Some(110.0), Some(90.0)), "lower_band_touch");
    }

    #[test]
    fn boll_inside() {
        assert_eq!(boll_signal(Some(100.0), Some(110.0), Some(90.0)), "inside_band");
    }

    #[test]
    fn boll_unavailable() {
        assert_eq!(boll_signal(None, None, None), "unavailable");
    }

    // --- atr_signal ---

    #[test]
    fn atr_high_volatility() {
        assert_eq!(atr_signal(Some(5.0), Some(100.0)), "high_volatility");
    }

    #[test]
    fn atr_low_volatility() {
        assert_eq!(atr_signal(Some(1.0), Some(100.0)), "low_volatility");
    }

    #[test]
    fn atr_normal() {
        assert_eq!(atr_signal(Some(2.5), Some(100.0)), "normal_volatility");
    }

    #[test]
    fn atr_unavailable() {
        assert_eq!(atr_signal(None, None), "unavailable");
    }

    // --- boll_width_signal ---

    #[test]
    fn boll_width_expanding() {
        assert_eq!(boll_width_signal(Some(15.0)), "band_expanding");
    }

    #[test]
    fn boll_width_squeezing() {
        assert_eq!(boll_width_signal(Some(3.0)), "band_squeezing");
    }

    #[test]
    fn boll_width_normal() {
        assert_eq!(boll_width_signal(Some(8.0)), "band_normal");
    }

    #[test]
    fn boll_width_unavailable() {
        assert_eq!(boll_width_signal(None), "unavailable");
    }

    // --- derive_technical_conclusions ---

    #[test]
    fn conclusions_rsi_overheated() {
        let values = TechnicalValues { rsi: Some(80.0), ..Default::default() };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "technical_overheated"));
    }

    #[test]
    fn conclusions_rsi_oversold() {
        let values = TechnicalValues { rsi: Some(25.0), ..Default::default() };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "technical_oversold"));
    }

    #[test]
    fn conclusions_rsi_neutral() {
        let values = TechnicalValues { rsi: Some(50.0), ..Default::default() };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "technical_neutral_zone"));
    }

    #[test]
    fn conclusions_trend_structure_positive() {
        let values = TechnicalValues {
            ma50: Some(95.0),
            ma200: Some(90.0),
            ..Default::default()
        };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "trend_structure_positive"));
    }

    #[test]
    fn conclusions_death_cross() {
        let values = TechnicalValues {
            ma50: Some(85.0),
            ma200: Some(90.0),
            ..Default::default()
        };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "death_cross"));
    }

    #[test]
    fn conclusions_volatility_elevated() {
        let values = TechnicalValues {
            atr: Some(5.0),
            ..Default::default()
        };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "volatility_elevated"));
    }

    #[test]
    fn conclusions_volume_confirms_bid() {
        let values = TechnicalValues {
            obv_delta: Some(1000.0),
            ..Default::default()
        };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "volume_confirms_bid"));
    }

    #[test]
    fn conclusions_macd_bullish() {
        let values = TechnicalValues {
            macd: Some(0.5),
            macd_hist: Some(0.3),
            ..Default::default()
        };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "macd_bullish"));
    }

    #[test]
    fn conclusions_macd_bearish() {
        let values = TechnicalValues {
            macd: Some(-0.5),
            macd_hist: Some(-0.3),
            ..Default::default()
        };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "macd_bearish"));
    }

    #[test]
    fn conclusions_price_below_ma50() {
        let values = TechnicalValues {
            ma50: Some(105.0),
            ..Default::default()
        };
        let conclusions = derive_technical_conclusions(&values, Some(100.0));
        assert!(conclusions.iter().any(|c| c.key == "price_below_ma50"));
    }

    #[test]
    fn conclusions_empty_values() {
        let values = TechnicalValues::default();
        let conclusions = derive_technical_conclusions(&values, None);
        assert!(conclusions.is_empty());
    }
}

fn normalize_execution_references(
    result: &AnalysisResult,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &mut StructuredPortfolioDecision,
) {
    let current_price = latest_market_close(result);
    let price_anchors = collect_price_anchors(result, trader_plan, portfolio_decision);
    let parsed_target = parse_first_numeric(&portfolio_decision.price_target);
    let rating = fallback_rating(portfolio_decision);
    let target_is_valid = parsed_target.is_some_and(|target| {
        target_passes_sanity_checks(target, current_price, &rating, &price_anchors)
    });

    if !target_is_valid {
        portfolio_decision.price_target.clear();
    }

    if portfolio_decision.confirmation_level.trim().is_empty() {
        portfolio_decision.confirmation_level = if !trader_plan.confirmation_level.trim().is_empty()
        {
            trader_plan.confirmation_level.trim().to_string()
        } else {
            rebuild_confirmation_level(current_price, &price_anchors, trader_plan, portfolio_decision)
        };
    }

    if portfolio_decision.price_target.trim().is_empty() {
        portfolio_decision.price_target = if !trader_plan.target_reference.trim().is_empty()
            && parse_first_numeric(&trader_plan.target_reference).is_some()
        {
            trader_plan.target_reference.trim().to_string()
        } else {
            rebuild_directional_target(current_price, &price_anchors, trader_plan, portfolio_decision)
        };
    }

    if portfolio_decision.target_reference.trim().is_empty() {
        if !trader_plan.target_reference.trim().is_empty() {
            portfolio_decision.target_reference = trader_plan.target_reference.trim().to_string();
        } else if !portfolio_decision.price_target.trim().is_empty() {
            portfolio_decision.target_reference = portfolio_decision.price_target.trim().to_string();
        }
    }

    if portfolio_decision.target_condition.trim().is_empty()
        && !trader_plan.target_condition.trim().is_empty()
    {
        portfolio_decision.target_condition = trader_plan.target_condition.trim().to_string();
    }

    if portfolio_decision.time_horizon.trim().is_empty()
        && !trader_plan.time_horizon.trim().is_empty()
    {
        portfolio_decision.time_horizon = trader_plan.time_horizon.trim().to_string();
    }
}

fn latest_market_close(result: &AnalysisResult) -> Option<f64> {
    if let Some(close) = result
        .artifacts
        .market_chart
        .candles
        .last()
        .map(|item| item.close)
        .filter(|value| value.is_finite() && *value > 0.0)
    {
        return Some(close);
    }
    derive_market_reference_facts(result)
        .into_iter()
        .find(|item| item.key == "latest_close")
        .and_then(|item| parse_first_numeric(&item.value))
}

fn reference_numeric(references: &ReportReferenceSnapshot, key: &str) -> Option<f64> {
    references
        .market
        .iter()
        .find(|item| item.key == key)
        .and_then(|item| parse_first_numeric(&item.value))
}

fn add_overlay(overlays: &mut Vec<ChartOverlay>, key: &str, value: Option<f64>, emphasis: &str) {
    let Some(value) = value.filter(|item| item.is_finite() && *item > 0.0) else {
        return;
    };
    if overlays.iter().any(|item| item.key == key) {
        return;
    }
    overlays.push(ChartOverlay {
        key: key.to_string(),
        value,
        emphasis: emphasis.to_string(),
    });
}

fn enrich_market_chart(
    chart: &mut ReportMarketChart,
    references: &ReportReferenceSnapshot,
    decision: &DecisionView,
) {
    if chart.indicators.is_empty() {
        chart.indicators = references
            .market
            .iter()
            .filter(|item| item.key != "latest_close" && item.key != "window_return")
            .cloned()
            .collect();
    }
    let mut overlays = Vec::new();
    add_overlay(
        &mut overlays,
        "current_price",
        parse_first_numeric(&decision.current_price).or_else(|| reference_numeric(references, "latest_close")),
        "primary",
    );
    add_overlay(
        &mut overlays,
        "confirmation_price",
        parse_first_numeric(&decision.confirmation_price),
        "success",
    );
    add_overlay(
        &mut overlays,
        "invalidation_price",
        parse_first_numeric(&decision.invalidation_price),
        "warning",
    );
    add_overlay(
        &mut overlays,
        "target_price",
        parse_first_numeric(decision.target_reference.as_str()),
        "info",
    );
    add_overlay(
        &mut overlays,
        "sma_50",
        reference_numeric(references, "close_50_sma"),
        "info",
    );
    add_overlay(
        &mut overlays,
        "ema_10",
        reference_numeric(references, "close_10_ema"),
        "info",
    );
    chart.overlays = overlays;
    chart.trend_lines = compute_trend_lines(&chart.candles);
}

fn compute_trend_lines(candles: &[ReportCandle]) -> Vec<TrendLine> {
    if candles.len() < 50 {
        return Vec::new();
    }
    let mut lines = Vec::new();

    // SMA series helper
    fn sma_series(candles: &[ReportCandle], period: usize, key: &str, color: &str) -> Option<TrendLine> {
        if candles.len() < period {
            return None;
        }
        let points: Vec<TrendLinePoint> = candles
            .windows(period)
            .enumerate()
            .map(|(i, window)| {
                let avg = window.iter().map(|c| c.close).sum::<f64>() / period as f64;
                TrendLinePoint {
                    date: candles[i + period - 1].trade_date.clone(),
                    value: avg,
                }
            })
            .collect();
        Some(TrendLine {
            key: key.to_string(),
            color: color.to_string(),
            points,
        })
    }

    if let Some(line) = sma_series(candles, 50, "sma_50", "#3b82f6") {
        lines.push(line);
    }
    if let Some(line) = sma_series(candles, 200, "sma_200", "#f59e0b") {
        lines.push(line);
    }

    // Linear regression on last 252 days
    let lookback = candles.len().min(252);
    if lookback >= 10 {
        let window = &candles[candles.len() - lookback..];
        let n = window.len() as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_xx = 0.0;
        for (i, candle) in window.iter().enumerate() {
            let x = i as f64;
            let y = candle.close;
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
        }
        let denom = n * sum_xx - sum_x * sum_x;
        if denom.abs() > f64::EPSILON {
            let slope = (n * sum_xy - sum_x * sum_y) / denom;
            let intercept = (sum_y - slope * sum_x) / n;
            let points: Vec<TrendLinePoint> = window
                .iter()
                .enumerate()
                .map(|(i, candle)| TrendLinePoint {
                    date: candle.trade_date.clone(),
                    value: slope * i as f64 + intercept,
                })
                .collect();
            lines.push(TrendLine {
                key: "regression".to_string(),
                color: "#8b5cf6".to_string(),
                points,
            });
        }
    }

    lines
}

fn derive_price_context(chart: &ReportMarketChart, current_price: Option<f64>) -> PriceContext {
    let lookback = chart.candles.len().min(252);
    let window = if lookback > 0 {
        &chart.candles[chart.candles.len() - lookback..]
    } else {
        &[][..]
    };
    let high = window
        .iter()
        .max_by(|left, right| left.high.partial_cmp(&right.high).unwrap_or(std::cmp::Ordering::Equal));
    let low = window
        .iter()
        .min_by(|left, right| left.low.partial_cmp(&right.low).unwrap_or(std::cmp::Ordering::Equal));
    let current = current_price.or_else(|| chart.candles.last().map(|item| item.close));
    let high_price = high.map(|item| item.high);
    let low_price = low.map(|item| item.low);
    let distance_to_high_pct = current.zip(high_price).and_then(|(current, high)| {
        (current > 0.0 && high >= current).then_some(((high - current) / current) * 100.0)
    });
    let distance_to_low_pct = current.zip(low_price).and_then(|(current, low)| {
        (current > 0.0 && low <= current).then_some(((current - low) / current) * 100.0)
    });
    let range_pct = high_price.zip(low_price).and_then(|(high, low)| {
        (low > 0.0 && high >= low).then_some(((high - low) / low) * 100.0)
    });
    let latest_volume = chart.candles.last().map(|item| item.volume);
    let avg_volume = if window.len() > 1 {
        let sum = window.iter().map(|item| item.volume as f64).sum::<f64>();
        Some(sum / window.len() as f64)
    } else {
        None
    };
    let volume_change_pct = latest_volume
        .zip(avg_volume.map(|item| item as i64))
        .and_then(|(latest, average)| {
            (average > 0).then_some(((latest as f64 - average as f64) / average as f64) * 100.0)
        });

    PriceContext {
        current_price: current,
        lookback_days: lookback,
        high_price,
        high_date: high.map(|item| item.trade_date.clone()).unwrap_or_default(),
        low_price,
        low_date: low.map(|item| item.trade_date.clone()).unwrap_or_default(),
        distance_to_high_pct,
        distance_to_low_pct,
        range_pct,
        latest_volume,
        volume_change_pct,
    }
}

#[cfg(test)]
mod chart_logic_tests {
    use super::super::*;

    fn make_candle(date: &str, close: f64, high: f64, low: f64, volume: i64) -> ReportCandle {
        ReportCandle {
            trade_date: date.into(),
            open: close,
            close,
            high,
            low,
            volume,
            ..Default::default()
        }
    }

    fn sample_candles(n: usize) -> Vec<ReportCandle> {
        (0..n)
            .map(|i| {
                let date = format!("2026-{:02}-{:02}", (i / 28) + 1, (i % 28) + 1);
                let close = 100.0 + (i as f64) * 0.5;
                make_candle(&date, close, close + 2.0, close - 2.0, 1000 + i as i64 * 10)
            })
            .collect()
    }

    #[test]
    fn compute_trend_lines_insufficient_data() {
        let candles = sample_candles(30);
        let lines = compute_trend_lines(&candles);
        assert!(lines.is_empty());
    }

    #[test]
    fn compute_trend_lines_with_sma50() {
        let candles = sample_candles(60);
        let lines = compute_trend_lines(&candles);
        let sma50 = lines.iter().find(|l| l.key == "sma_50");
        assert!(sma50.is_some(), "expected sma_50 trend line");
    }

    #[test]
    fn compute_trend_lines_with_regression() {
        let candles = sample_candles(60);
        let lines = compute_trend_lines(&candles);
        let reg = lines.iter().find(|l| l.key == "regression");
        assert!(reg.is_some(), "expected regression trend line");
    }

    #[test]
    fn compute_trend_lines_regression_points_count() {
        let candles = sample_candles(100);
        let lines = compute_trend_lines(&candles);
        let reg = lines.iter().find(|l| l.key == "regression").unwrap();
        assert_eq!(reg.points.len(), 100);
    }

    #[test]
    fn add_overlay_skips_non_positive() {
        let mut overlays = Vec::new();
        add_overlay(&mut overlays, "test", Some(-1.0), "info");
        assert!(overlays.is_empty());
    }

    #[test]
    fn add_overlay_skips_nan() {
        let mut overlays = Vec::new();
        add_overlay(&mut overlays, "test", Some(f64::NAN), "info");
        assert!(overlays.is_empty());
    }

    #[test]
    fn add_overlay_skips_none() {
        let mut overlays = Vec::new();
        add_overlay(&mut overlays, "test", None, "info");
        assert!(overlays.is_empty());
    }

    #[test]
    fn add_overlay_adds_valid() {
        let mut overlays = Vec::new();
        add_overlay(&mut overlays, "price", Some(150.0), "primary");
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].key, "price");
        assert_eq!(overlays[0].value, 150.0);
    }

    #[test]
    fn add_overlay_deduplicates() {
        let mut overlays = Vec::new();
        add_overlay(&mut overlays, "price", Some(150.0), "primary");
        add_overlay(&mut overlays, "price", Some(160.0), "info");
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].value, 150.0);
    }

    #[test]
    fn derive_price_context_empty_chart() {
        let chart = ReportMarketChart::default();
        let ctx = derive_price_context(&chart, None);
        assert!(ctx.current_price.is_none());
        assert!(ctx.high_price.is_none());
    }

    #[test]
    fn derive_price_context_with_candles() {
        let candles = vec![
            make_candle("2026-01-01", 100.0, 105.0, 95.0, 1000),
            make_candle("2026-01-02", 110.0, 115.0, 105.0, 1200),
            make_candle("2026-01-03", 105.0, 112.0, 98.0, 800),
        ];
        let chart = ReportMarketChart {
            candles,
            ..Default::default()
        };
        let ctx = derive_price_context(&chart, None);
        assert_eq!(ctx.current_price, Some(105.0));
        assert_eq!(ctx.high_price, Some(115.0));
        assert_eq!(ctx.low_price, Some(95.0));
    }

    #[test]
    fn derive_price_context_distance_calculations() {
        let candles = vec![
            make_candle("2026-01-01", 100.0, 100.0, 100.0, 1000),
            make_candle("2026-01-02", 110.0, 120.0, 90.0, 1000),
        ];
        let chart = ReportMarketChart {
            candles,
            ..Default::default()
        };
        let ctx = derive_price_context(&chart, None);
        // current=110, high=120, low=90
        // distance_to_high = (120-110)/110 * 100 = 9.09%
        assert!(ctx.distance_to_high_pct.unwrap() > 8.0);
        assert!(ctx.distance_to_low_pct.unwrap() > 18.0);
    }

    #[test]
    fn derive_price_context_override_current_price() {
        let candles = vec![make_candle("2026-01-01", 100.0, 105.0, 95.0, 1000)];
        let chart = ReportMarketChart {
            candles,
            ..Default::default()
        };
        let ctx = derive_price_context(&chart, Some(200.0));
        assert_eq!(ctx.current_price, Some(200.0));
    }
}


fn is_price_field_empty(field: &str) -> bool {
    let trimmed = field.trim();
    trimmed.is_empty()
        || trimmed == "待分析"
        || trimmed == "待确认"
        || trimmed == "N/A"
        || trimmed == "n/a"
        || trimmed == "-"
        || trimmed == "--"
}

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
        // Aggressive target replacement: try to find a valid replacement from
        // anchors before clearing, to avoid losing target information entirely.
        let replacement = rebuild_directional_target(current_price, &price_anchors, trader_plan, portfolio_decision);
        if !replacement.is_empty() {
            let replacement_valid = parse_first_numeric(&replacement).is_some_and(|target| {
                target_passes_sanity_checks(target, current_price, &rating, &price_anchors)
            });
            if replacement_valid {
                portfolio_decision.price_target = replacement;
            } else {
                portfolio_decision.price_target.clear();
            }
        } else {
            portfolio_decision.price_target.clear();
        }
    }

    if is_price_field_empty(&portfolio_decision.confirmation_level) {
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

pub fn add_overlay(overlays: &mut Vec<ChartOverlay>, key: &str, value: Option<f64>, emphasis: &str) {
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

pub fn compute_trend_lines(candles: &[ReportCandle]) -> Vec<TrendLine> {
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

pub fn derive_price_context(chart: &ReportMarketChart, current_price: Option<f64>) -> PriceContext {
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

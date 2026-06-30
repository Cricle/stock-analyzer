fn clamp_probability(value: f64) -> f64 {
    value.clamp(5.0, 90.0)
}

/// Round three probability components and adjust the largest so the sum is
/// exactly 100.  Rounding can drift by +/-1, so we add the residual to the
/// largest component.
pub fn round_to_100(upside: f64, downside: f64, sideways: f64) -> (f64, f64, f64) {
    let mut u = upside.round();
    let mut d = downside.round();
    let mut s = sideways.round();
    let err = 100.0 - u - d - s;
    if err.abs() > f64::EPSILON {
        if u >= d && u >= s {
            u += err;
        } else if d >= s {
            d += err;
        } else {
            s += err;
        }
    }
    (u, d, s)
}

fn derive_probability_view(
    decision: &DecisionView,
    direction_score: i32,
    confidence_score: i32,
    price_context: &PriceContext,
    memory_context: &MemoryContextSnapshot,
    technical_indicators: &TechnicalIndicatorView,
) -> ProbabilityView {
    let confidence = (confidence_score as f64 / 100.0).clamp(0.0, 1.0);
    let directional_bias = (direction_score as f64 / 100.0).clamp(-1.0, 1.0);
    let memory_alpha = memory_context.setup_match_avg_alpha_return;
    let memory_hit = memory_context.setup_match_hit_rate;
    let mut upside = 34.0 + directional_bias * 24.0 + (confidence - 0.5) * 16.0;
    if memory_context.setup_resolved_match_count > 0 {
        upside += (memory_hit - 0.5) * 14.0 + memory_alpha * 100.0;
    }
    if matches!(decision.action_bias, DecisionActionBias::NoTrade) {
        upside -= 6.0;
    }
    let upside = clamp_probability(upside);
    let downside = clamp_probability(32.0 - directional_bias * 18.0 + (1.0 - confidence) * 12.0);
    let sideways = (100.0 - upside - downside).clamp(5.0, 70.0);
    let total = upside + downside + sideways;
    let scale = if total > 0.0 { 100.0 / total } else { 1.0 };
    let (upside, downside, sideways) =
        round_to_100(upside * scale, downside * scale, sideways * scale);
    let risk_probability = (downside + (100.0 - confidence_score as f64) * 0.2).clamp(5.0, 90.0);
    let current = price_context.current_price;
    let is_bearish = matches!(decision.view, DecisionViewDirection::Bearish);
    let upside_target = parse_first_numeric(decision.target_reference.as_str())
        .or(price_context.high_price)
        .filter(|value| value.is_finite() && *value > 0.0);
    let downside_target = parse_first_numeric(&decision.invalidation_price)
        .or(price_context.low_price)
        .filter(|value| value.is_finite() && *value > 0.0);
    let upside_pct = current.zip(upside_target).and_then(|(current, target)| {
        if is_bearish {
            // Bearish: profit is stock going DOWN (target < current)
            (current > 0.0 && target < current).then_some(((current - target) / current) * 100.0)
        } else {
            // Bullish/Neutral: profit is stock going UP (target > current)
            (current > 0.0 && target > current).then_some(((target - current) / current) * 100.0)
        }
    });
    let downside_pct = current.zip(downside_target).and_then(|(current, target)| {
        if is_bearish {
            // Bearish: loss is stock going UP (stop > current)
            (current > 0.0 && target > current).then_some(((target - current) / current) * 100.0)
        } else {
            // Bullish/Neutral: loss is stock going DOWN (stop < current)
            (current > 0.0 && target < current).then_some(((current - target) / current) * 100.0)
        }
    });
    let mut drivers = vec![
        ProbabilityDriver {
            key: "direction_score".to_string(),
            direction: if direction_score >= 0 { "positive" } else { "negative" }.to_string(),
            value: direction_score.to_string(),
            evidence_keys: vec!["direction_breakdown".to_string()],
        },
        ProbabilityDriver {
            key: "confidence_score".to_string(),
            direction: if confidence_score >= 50 { "positive" } else { "caution" }.to_string(),
            value: confidence_score.to_string(),
            evidence_keys: vec!["confidence_profile".to_string()],
        },
    ];
    if memory_context.setup_resolved_match_count > 0 {
        drivers.push(ProbabilityDriver {
            key: "historical_setup".to_string(),
            direction: if memory_alpha > 0.0 { "positive" } else { "caution" }.to_string(),
            value: format!(
                "{} samples / {:.0}% hit / {:.1}% alpha",
                memory_context.setup_resolved_match_count,
                memory_hit * 100.0,
                memory_alpha * 100.0
            ),
            evidence_keys: vec!["memory".to_string()],
        });
    }
    // Add specific technical indicator drivers
    for category in &technical_indicators.categories {
        for indicator in &category.indicators {
            if let Some(value) = indicator.value {
                let (direction, evidence_key) = match indicator.signal_code.as_str() {
                    "bullish_cross" | "above_reference" | "oversold" => ("positive", indicator.key.clone()),
                    "bearish_cross" | "below_reference" | "overbought" => ("negative", indicator.key.clone()),
                    _ => ("neutral", indicator.key.clone()),
                };
                // Only add key indicators to keep drivers compact
                if ["rsi", "macd", "adx", "kdj_j"].contains(&indicator.key.as_str()) {
                    drivers.push(ProbabilityDriver {
                        key: indicator.key.clone(),
                        direction: direction.to_string(),
                        value: format!("{:.2}", value),
                        evidence_keys: vec![evidence_key],
                    });
                }
            }
        }
    }

    ProbabilityView {
        upside_probability_pct: upside,
        upside_target,
        upside_pct,
        downside_probability_pct: downside,
        downside_target,
        downside_pct,
        sideways_probability_pct: sideways,
        risk_probability_pct: risk_probability.round(),
        confidence_band: LocalText::new(match decision.confidence_band {
            DecisionConfidenceBand::High => "high",
            DecisionConfidenceBand::Medium => "medium",
            DecisionConfidenceBand::Low => "low",
        }),
        drivers,
    }
}

fn derive_profit_risk(
    decision: &DecisionView,
    price_context: &PriceContext,
    probability: &ProbabilityView,
) -> ProfitRiskView {
    let is_bearish = matches!(decision.view, DecisionViewDirection::Bearish);
    let reward_risk_ratio = probability
        .upside_pct
        .zip(probability.downside_pct)
        .and_then(|(up, down)| (down > 0.0).then_some(up / down))
        .or_else(|| {
            let current = parse_first_numeric(&decision.current_price)
                .or(price_context.current_price)?;
            let target = parse_first_numeric(decision.target_reference.as_str())?;
            let stop = parse_first_numeric(&decision.invalidation_price)
                .or(price_context.low_price)?;
            if is_bearish {
                // Bearish: target < current (profit down), stop > current (loss up)
                if current > 0.0 && stop > current && target < current {
                    Some((current - target) / (stop - current))
                } else {
                    None
                }
            } else {
                // Bullish/Neutral: target > current (profit up), stop < current (loss down)
                if current > 0.0 && stop < current && target > current {
                    Some((target - current) / (current - stop))
                } else {
                    None
                }
            }
        });
    let upside_pct = probability.upside_pct.or_else(|| {
        let current = parse_first_numeric(&decision.current_price)
            .or(price_context.current_price)?;
        let target = parse_first_numeric(decision.target_reference.as_str())?;
        if is_bearish {
            // Bearish: profit is stock going DOWN
            if current > 0.0 && target < current {
                Some(((current - target) / current) * 100.0)
            } else {
                None
            }
        } else {
            if current > 0.0 && target > current {
                Some(((target - current) / current) * 100.0)
            } else {
                None
            }
        }
    });
    let downside_pct = probability.downside_pct.or_else(|| {
        let current = parse_first_numeric(&decision.current_price)
            .or(price_context.current_price)?;
        let stop = parse_first_numeric(&decision.invalidation_price)
            .or(price_context.low_price)?;
        if is_bearish {
            // Bearish: loss is stock going UP
            if current > 0.0 && stop > current {
                Some(((stop - current) / current) * 100.0)
            } else {
                None
            }
        } else {
            if current > 0.0 && stop < current {
                Some(((current - stop) / current) * 100.0)
            } else {
                None
            }
        }
    });
    // Current-position ratio: uses confirmation_price as upside (pre-breakout),
    // vs post-breakout ratio which uses target_reference.
    let current_position_reward_risk_ratio = {
        let current = parse_first_numeric(&decision.current_price)
            .or(price_context.current_price);
        let confirmation = parse_first_numeric(&decision.confirmation_price);
        let stop = parse_first_numeric(&decision.invalidation_price)
            .or(price_context.low_price);
        match (current, confirmation, stop) {
            (Some(c), Some(conf), Some(s)) if c > 0.0 && conf > c && s < c => {
                Some(((conf - c) / c * 100.0) / ((c - s) / c * 100.0))
            }
            _ => None,
        }
    };
    ProfitRiskView {
        upside_pct,
        downside_pct,
        reward_risk_ratio,
        current_position_reward_risk_ratio,
        max_loss_reference: parse_first_numeric(&decision.invalidation_price).or(price_context.low_price),
        risk_budget: decision.sizing_guidance.clone(),
        actionability: LocalText::new(decision_action_code(&decision.action)),
    }
}

fn derive_ic_navigator(decision: &DecisionView, probability: &ProbabilityView) -> IcNavigatorView {
    let can_act_now = matches!(decision.action, DecisionAction::BuyNow | DecisionAction::ProbePosition);
    IcNavigatorView {
        verdict: LocalText::new(decision_action_code(&decision.action)),
        primary_path_key: if decision.primary_path_key.trim().is_empty() {
            "base_case".to_string()
        } else {
            decision.primary_path_key.clone()
        },
        path_probability_pct: probability.upside_probability_pct,
        confidence_band: match decision.confidence_band {
            DecisionConfidenceBand::High => "high",
            DecisionConfidenceBand::Medium => "medium",
            DecisionConfidenceBand::Low => "low",
        }
        .to_string(),
        can_act_now,
        early_probe_allowed: decision.early_probe_allowed,
        upgrade_condition: decision.next_upgrade_condition.clone(),
        abort_condition: decision.abort_plan.clone(),
        responsibility: if can_act_now {
            LocalText::new("ic_chair_accepts_probe_risk")
        } else {
            LocalText::new("ic_chair_requires_confirmation")
        },
    }
}

fn derive_ic_discipline(
    decision: &DecisionView,
    chart: &ReportMarketChart,
    technical_indicators: &TechnicalIndicatorView,
    price_context: &PriceContext,
    probability: &ProbabilityView,
    profit_risk: &ProfitRiskView,
) -> IcDisciplineView {
    let reward_risk_ratio = profit_risk.reward_risk_ratio;
    let rsi = indicator_value(chart, "rsi");
    let macd = indicator_value(chart, "macd");
    let current_price = price_context.current_price;
    let confirmation_price = parse_first_numeric(&decision.confirmation_price)
        .or(probability.upside_target);
    let invalidation_price = parse_first_numeric(&decision.invalidation_price)
        .or(probability.downside_target)
        .or(price_context.low_price);
    let confirmation_met = confirmation_price
        .zip(current_price)
        .is_some_and(|(confirmation, current)| current >= confirmation);
    let invalidation_broken = invalidation_price
        .zip(current_price)
        .is_some_and(|(invalidation, current)| current <= invalidation);
    let poor_reward_risk = reward_risk_ratio.is_none_or(|value| value < 0.5);
    let overheated_rsi = rsi.is_some_and(|value| value > 75.0);
    let confirmation_missing = !confirmation_met;
    let risk_probability_high =
        probability.risk_probability_pct > probability.upside_probability_pct + 10.0;
    let severe_risk_probability =
        probability.risk_probability_pct >= probability.upside_probability_pct + 25.0
            && probability.downside_probability_pct >= probability.upside_probability_pct + 15.0;

    let state = if invalidation_broken || severe_risk_probability {
        "must_defend"
    } else if poor_reward_risk || overheated_rsi {
        "no_attack"
    } else if reward_risk_ratio.is_some_and(|value| value >= 1.2) && confirmation_met {
        "attack_allowed"
    } else {
        "probe_watch"
    };

    let mut reason_codes = Vec::new();
    if poor_reward_risk {
        reason_codes.push("poor_reward_risk".to_string());
    }
    if overheated_rsi {
        reason_codes.push("overheated_rsi".to_string());
    }
    if confirmation_missing {
        reason_codes.push("confirmation_missing".to_string());
    }
    if risk_probability_high {
        reason_codes.push("risk_probability_high".to_string());
    }
    if invalidation_broken {
        reason_codes.push("invalidation_broken".to_string());
    }
    let technical_signal_codes = technical_indicators
        .conclusions
        .iter()
        .map(|item| item.key.clone())
        .collect::<Vec<_>>();
    for code in &technical_signal_codes {
        if matches!(code.as_str(), "technical_overheated" | "macd_momentum_lag") {
            reason_codes.push(code.clone());
        }
    }
    reason_codes.sort();
    reason_codes.dedup();

    let next_action_code = match state {
        "must_defend" => "defend_or_exit",
        "attack_allowed" => "allow_attack",
        "probe_watch" if decision.early_probe_allowed => "allow_small_probe",
        "probe_watch" => "wait_breakout_confirmation",
        "no_attack" if overheated_rsi => "wait_cooling",
        _ => "wait_breakout_confirmation",
    };

    IcDisciplineView {
        state: LocalText::new(state),
        reason_codes,
        next_action_code: LocalText::new(next_action_code),
        reward_risk_ratio,
        current_position_reward_risk_ratio: profit_risk.current_position_reward_risk_ratio,
        rsi,
        macd,
        upside_probability_pct: probability.upside_probability_pct,
        downside_probability_pct: probability.downside_probability_pct,
        risk_probability_pct: probability.risk_probability_pct,
        current_price,
        confirmation_price,
        invalidation_price,
        upside_pct: probability.upside_pct,
        downside_pct: probability.downside_pct,
        technical_signal_codes,
    }
}

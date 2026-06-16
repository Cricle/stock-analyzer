use crate::engine::stock_pick::{EnrichedCandidate, FactorBreakdown};

pub(super) fn compute_factor_breakdown(item: &EnrichedCandidate) -> FactorBreakdown {
    let momentum = momentum_score(item);
    let quality = quality_score(item);
    let value = value_score(item);
    let growth = growth_score(item);
    let profitability = profitability_score(item);
    let risk = risk_score(item);
    let event = event_score(item);
    let evidence = evidence_score(item);
    let history = history_score(item);
    let penalty = penalty_score(item);
    let total = (0.18 * momentum
        + 0.14 * quality
        + 0.12 * value
        + 0.12 * growth
        + 0.10 * profitability
        + 0.10 * risk
        + 0.08 * event
        + 0.08 * evidence
        + 0.08 * history
        + penalty)
        .clamp(0.0, 100.0);

    FactorBreakdown {
        momentum,
        quality,
        value,
        growth,
        profitability,
        risk,
        event,
        evidence,
        history,
        penalty,
        total,
    }
}

fn momentum_score(item: &EnrichedCandidate) -> f64 {
    let Some(first) = item.candles.first() else {
        return 0.0;
    };
    let Some(last) = item.candles.last() else {
        return 0.0;
    };
    if first.close <= 0.0 {
        return 0.0;
    }
    let return_pct = ((last.close / first.close) - 1.0) * 100.0;
    let volume_ratio = if first.volume > 0 {
        last.volume as f64 / first.volume as f64
    } else {
        1.0
    };
    let smoothness = item
        .candles
        .windows(2)
        .filter(|pair| pair[1].close >= pair[0].close)
        .count() as f64
        / item.candles.windows(2).count().max(1) as f64;

    let mut score = 45.0
        + return_pct.clamp(-10.0, 25.0) * 1.2
        + (volume_ratio.min(4.0) - 1.0).max(0.0) * 8.0
        + smoothness * 12.0;

    // RSI bonus: 50-70 is bullish momentum without overbought
    if let Some(rsi) = item.technical_snapshot.rsi {
        if (55.0..70.0).contains(&rsi) {
            score += 6.0;
        } else if rsi > 75.0 {
            score -= 4.0; // Overbought penalty
        } else if rsi < 30.0 {
            score -= 6.0; // Oversold, potential reversal risk
        }
    }
    // MACD histogram positive = uptrend confirmation
    if let Some(macd) = item.technical_snapshot.macd_hist {
        if macd > 0.0 {
            score += 5.0;
        } else if macd < -0.5 {
            score -= 3.0;
        }
    }
    // ADX: strong trend (>25) is good for momentum strategies
    if let Some(adx) = item.technical_snapshot.adx {
        if adx > 30.0 {
            score += 4.0;
        } else if adx < 15.0 {
            score -= 2.0; // No clear trend
        }
    }
    // Price above 50-day SMA = medium-term uptrend
    if let (Some(price), Some(sma50)) = (item.price, item.technical_snapshot.close_50_sma)
        && price > sma50
        && sma50 > 0.0
    {
        score += 3.0;
    }
    // KDJ: J > K > D and J < 80 = bullish momentum without overbought
    if let (Some(k), Some(d), Some(j)) = (
        item.technical_snapshot.kdj_k,
        item.technical_snapshot.kdj_d,
        item.technical_snapshot.kdj_j,
    ) {
        if j > k && k > d && j < 80.0 {
            score += 3.0; // Bullish KDJ crossover
        } else if j > 90.0 {
            score -= 2.0; // Overbought
        }
    }
    // CCI: 0-100 = bullish momentum
    if let Some(cci) = item.technical_snapshot.cci {
        if (0.0..100.0).contains(&cci) {
            score += 2.0;
        } else if cci > 200.0 {
            score -= 2.0; // Extremely overbought
        }
    }
    // Williams %R: -20 to 0 = overbought, -80 to -100 = oversold
    if let Some(wr) = item.technical_snapshot.wr {
        if (-50.0..-20.0).contains(&wr) {
            score += 2.0; // Bullish zone
        } else if wr > -10.0 {
            score -= 2.0; // Overbought
        } else if wr < -90.0 {
            score -= 2.0; // Oversold
        }
    }

    // Absolute cap: declining stocks shouldn't get top momentum scores
    let return_pct = item
        .candles
        .first()
        .zip(item.candles.last())
        .filter(|(f, _)| f.close > 0.0)
        .map(|(f, l)| ((l.close / f.close) - 1.0) * 100.0)
        .unwrap_or(0.0);
    let cap = if return_pct < -5.0 { 60.0 } else { 100.0 };
    score.clamp(0.0, cap)
}

fn quality_score(item: &EnrichedCandidate) -> f64 {
    let roe = item.fundamental_snapshot.roe.unwrap_or(0.0);
    let leverage = item.fundamental_snapshot.leverage.unwrap_or(1.0);
    let mut score = 55.0 + roe.clamp(-0.2, 0.4) * 80.0 - leverage.clamp(0.0, 3.0) * 8.0;
    // Bonus for high gross margin (pricing power indicator)
    if let Some(gm) = item.fundamental_snapshot.gross_margin {
        if gm > 0.4 {
            score += 6.0;
        } else if gm > 0.25 {
            score += 3.0;
        }
    }
    // FCF yield: FCF / market cap (higher is better, indicates cash generation efficiency)
    if let (Some(fcf), Some(mc)) = (
        item.fundamental_snapshot.free_cash_flow_usd.filter(|v| *v > 0.0),
        item.fundamental_snapshot.market_cap.filter(|v| *v > 0.0),
    ) {
        let fcf_yield = fcf / mc;
        if fcf_yield > 0.08 {
            score += 8.0;
        } else if fcf_yield > 0.04 {
            score += 4.0;
        }
    }
    // Dividend yield: consistent dividends signal financial stability
    if let Some(dy) = item.fundamental_snapshot.dividend_yield {
        if dy > 0.03 {
            score += 5.0;
        } else if dy > 0.01 {
            score += 2.0;
        }
    }
    // Analyst consensus: high buy ratio = quality validation
    if let Some(br) = item.fundamental_snapshot.analyst_buy_ratio {
        if br > 0.7 {
            score += 4.0;
        } else if br > 0.5 {
            score += 2.0;
        }
    }
    score.clamp(0.0, 100.0)
}

fn value_score(item: &EnrichedCandidate) -> f64 {
    // Prefer PE TTM from enrichment, fallback to computed PE
    let pe = item
        .fundamental_snapshot
        .pe_ttm
        .filter(|v| *v > 0.0)
        .or(item.fundamental_snapshot.pe_like)
        .unwrap_or(40.0);
    let ps = item
        .enrichment
        .ps
        .filter(|v| *v > 0.0)
        .or(item.fundamental_snapshot.ps_like)
        .unwrap_or(10.0);
    let mut score: f64 = 50.0;
    if pe < 15.0 {
        score += 18.0;
    } else if pe < 25.0 {
        score += 10.0;
    } else if pe > 60.0 {
        score -= 15.0;
    }
    if ps < 2.0 {
        score += 12.0;
    } else if ps > 8.0 {
        score -= 10.0;
    }
    // PB bonus for asset-backed value
    if let Some(pb) = item.fundamental_snapshot.pb {
        if pb < 1.0 {
            score += 10.0;
        } else if pb < 2.0 {
            score += 5.0;
        } else if pb > 8.0 {
            score -= 5.0;
        }
    }
    score.clamp(0.0, 100.0)
}

fn growth_score(item: &EnrichedCandidate) -> f64 {
    let mut score: f64 = 45.0;
    // Revenue growth
    if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy {
        if rev_yoy > 0.3 {
            score += 18.0;
        } else if rev_yoy > 0.15 {
            score += 12.0;
        } else if rev_yoy > 0.0 {
            score += 5.0;
        } else if rev_yoy < -0.15 {
            score -= 12.0;
        }
    }
    // Net profit growth
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy {
        if np_yoy > 0.5 {
            score += 20.0;
        } else if np_yoy > 0.2 {
            score += 12.0;
        } else if np_yoy > 0.0 {
            score += 5.0;
        } else if np_yoy < -0.2 {
            score -= 15.0;
        }
    }
    // PEG: lower is better (< 1 is undervalued growth)
    if let Some(peg) = item.fundamental_snapshot.peg {
        if peg > 0.0 && peg < 1.0 {
            score += 10.0;
        } else if peg > 0.0 && peg < 2.0 {
            score += 5.0;
        } else if peg > 3.0 {
            score -= 5.0;
        }
    }
    // Fund flow: positive net buying is bullish
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio {
        if flow > 0.1 {
            score += 8.0;
        } else if flow > 0.0 {
            score += 4.0;
        } else if flow < -0.1 {
            score -= 6.0;
        }
    }
    // Analyst coverage & consensus
    if let Some(count) = item.fundamental_snapshot.analyst_report_count {
        if count >= 10 {
            score += 6.0;
        } else if count >= 3 {
            score += 3.0;
        }
    }
    if let Some(buy_ratio) = item.fundamental_snapshot.analyst_buy_ratio {
        if buy_ratio > 0.7 {
            score += 8.0;
        } else if buy_ratio > 0.5 {
            score += 4.0;
        }
    }
    // Dividend yield: income signal
    if let Some(dy) = item.fundamental_snapshot.dividend_yield {
        if dy > 0.03 {
            score += 6.0;
        } else if dy > 0.01 {
            score += 3.0;
        }
    }
    score.clamp(0.0, 100.0)
}

fn profitability_score(item: &EnrichedCandidate) -> f64 {
    let margin = match (
        item.fundamental_snapshot.net_income_usd,
        item.fundamental_snapshot.revenues_usd.filter(|v| *v > 0.0),
    ) {
        (Some(ni), Some(rev)) => ni / rev,
        _ => 0.0,
    };
    let cash_conversion = match item.fundamentals.as_ref() {
        Some(f) => {
            let ocf = f.operating_cash_flow_usd;
            let ni = f.net_income_usd.filter(|v| v.abs() > 0.0);
            match (ocf, ni) {
                (Some(ocf), Some(ni)) => ocf / ni,
                _ => 0.0,
            }
        }
        None => 0.0,
    };
    let mut score = 48.0 + margin.clamp(-0.2, 0.3) * 90.0 + cash_conversion.clamp(-1.0, 2.0) * 8.0;
    // Gross margin bonus (from enrichment)
    if let Some(gm) = item.fundamental_snapshot.gross_margin {
        if gm > 0.5 {
            score += 8.0;
        } else if gm > 0.3 {
            score += 4.0;
        }
    }
    score.clamp(0.0, 100.0)
}

fn risk_score(item: &EnrichedCandidate) -> f64 {
    if item.candles.len() < 5 {
        return 35.0;
    }
    let avg_abs_change = item
        .candles
        .iter()
        .map(|item| item.change_pct.abs())
        .sum::<f64>()
        / item.candles.len() as f64;
    let latest_turnover = item
        .candles
        .last()
        .map(|item| item.turnover_pct)
        .unwrap_or(0.0);
    let mut score = 75.0 - avg_abs_change.clamp(0.0, 18.0) * 2.0 - latest_turnover.clamp(0.0, 40.0) * 0.5;
    // Chip benefit: high benefit ratio means most holders are in profit → less selloff risk
    if let Some(benefit) = item.fundamental_snapshot.chip_benefit_ratio {
        if benefit > 0.9 {
            score += 5.0; // Almost everyone profitable, low panic selling risk
        } else if benefit < 0.2 {
            score -= 5.0; // Most holders underwater, higher panic risk
        }
    }
    // Low chip concentration = more stable holder base
    if let Some(conc) = item.fundamental_snapshot.chip_concentration_90
        && conc < 0.15
    {
        score += 3.0; // Chips spread out, less whale manipulation
    }
    // Fund flow: heavy outflows increase risk
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio {
        if flow < -0.1 {
            score -= 6.0; // Institutional distribution
        } else if flow < -0.03 {
            score -= 3.0;
        } else if flow > 0.05 {
            score += 3.0; // Institutional accumulation, lower risk
        }
    }
    // High leverage increases risk
    if let Some(lev) = item.fundamental_snapshot.leverage {
        if lev > 2.0 {
            score -= 5.0;
        } else if lev > 1.5 {
            score -= 2.0;
        }
    }
    score.clamp(0.0, 100.0)
}

fn event_score(item: &EnrichedCandidate) -> f64 {
    let disclosure_count = item
        .news_snapshot
        .deep_item_count
        .max(item.news_snapshot.light_item_count) as f64;
    let recency_support = (!item.news_snapshot.latest_published_at.trim().is_empty()) as i32 as f64;
    let catalyst_support = item.news_snapshot.catalyst_count.min(4) as f64;
    let mut score = 35.0 + disclosure_count.min(8.0) * 4.0 + recency_support * 8.0 + catalyst_support * 6.0;
    // Fund flow as institutional event signal
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio {
        if flow > 0.05 {
            score += 6.0; // Strong institutional buying
        } else if flow > 0.0 {
            score += 3.0;
        }
    }
    // Analyst coverage as event catalyst
    if let Some(count) = item.fundamental_snapshot.analyst_report_count {
        if count >= 5 {
            score += 4.0;
        } else if count >= 2 {
            score += 2.0;
        }
    }
    score.clamp(0.0, 100.0)
}

fn evidence_score(item: &EnrichedCandidate) -> f64 {
    let evidence_count = item.evidence_records.len() as f64;
    let source_count = item.news_snapshot.unique_source_count as f64;
    let hard_negative_penalty = item.news_snapshot.hard_negative_count.min(4) as f64 * 10.0;
    (35.0 + evidence_count.min(12.0) * 4.5 + source_count.min(6.0) * 4.0 - hard_negative_penalty)
        .clamp(0.0, 100.0)
}

fn history_score(item: &EnrichedCandidate) -> f64 {
    let snapshot = &item.history_match_snapshot;
    if !snapshot.enabled {
        return 50.0;
    }
    let sample_component = snapshot.sample_count.min(12) as f64 * 4.0;
    let hit_component = snapshot.hit_rate.unwrap_or(0.5).clamp(0.0, 1.0) * 35.0;
    let alpha_component = snapshot
        .average_alpha_return
        .unwrap_or_default()
        .clamp(-0.2, 0.4)
        * 60.0;
    (20.0 + sample_component + hit_component + alpha_component).clamp(0.0, 100.0)
}

pub(super) fn penalty_score(item: &EnrichedCandidate) -> f64 {
    let mut penalty = 0.0;
    if let Some(change_pct) = item.change_pct {
        if change_pct >= 19.0 {
            penalty -= 10.0;
        } else if change_pct >= 9.5 {
            penalty -= 5.0;
        }
    }
    if item.market_cap.is_some_and(|value| value <= 0.0) {
        penalty -= 6.0;
    }
    if item
        .fundamentals
        .as_ref()
        .and_then(|f| f.revenues_usd)
        .is_some_and(|value| value <= 0.0)
    {
        penalty -= 5.0;
    }
    if item.risk_snapshot.volatility_elevated {
        penalty -= 4.0;
    }
    if item.risk_snapshot.liquidity_warning {
        penalty -= 4.0;
    }
    if item.risk_snapshot.valuation_stretched {
        penalty -= 3.0;
    }
    penalty
}

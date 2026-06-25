use crate::pick::{EnrichedCandidate, FactorBreakdown};

pub fn compute_factor_breakdown(item: &EnrichedCandidate) -> FactorBreakdown {
    let momentum = momentum_score(item);
    let quality = quality_score(item);
    let value = value_score(item);
    let profitability = profitability_score(item);
    let risk = risk_score(item);
    let event = event_score(item);
    let evidence = evidence_score(item);
    let history = history_score(item);
    let penalty = penalty_score(item);
    let total = (0.22 * momentum
        + 0.16 * quality
        + 0.12 * value
        + 0.12 * profitability
        + 0.10 * risk
        + 0.10 * event
        + 0.10 * evidence
        + 0.08 * history
        + penalty)
        .clamp(0.0, 100.0);

    FactorBreakdown {
        momentum,
        quality,
        value,
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

    (45.0
        + return_pct.clamp(-10.0, 25.0) * 1.2
        + (volume_ratio.min(4.0) - 1.0).max(0.0) * 8.0
        + smoothness * 12.0)
        .clamp(0.0, 100.0)
}

fn quality_score(item: &EnrichedCandidate) -> f64 {
    let Some(f) = item.fundamentals.as_ref() else {
        return 40.0;
    };
    let roe: f64 = match (
        f.net_income_usd,
        f.stockholders_equity_usd.filter(|value| *value > 0.0),
    ) {
        (Some(ni), Some(eq)) => ni / eq,
        _ => 0.0,
    };
    let leverage: f64 = match (
        f.total_debt_usd,
        f.stockholders_equity_usd.filter(|value| *value > 0.0),
    ) {
        (Some(debt), Some(eq)) => debt / eq,
        _ => 1.0,
    };
    (55.0 + roe.clamp(-0.2, 0.4) * 80.0 - leverage.clamp(0.0, 3.0) * 8.0).clamp(0.0, 100.0)
}

fn value_score(item: &EnrichedCandidate) -> f64 {
    let Some(f) = item.fundamentals.as_ref() else {
        return 45.0;
    };
    let pe_like: f64 = match (
        f.market_cap.filter(|value| *value > 0.0),
        f.net_income_usd.filter(|value| *value > 0.0),
    ) {
        (Some(mc), Some(ni)) => mc / ni,
        _ => 40.0,
    };
    let ps_like: f64 = match (
        f.market_cap.filter(|value| *value > 0.0),
        f.revenues_usd.filter(|value| *value > 0.0),
    ) {
        (Some(mc), Some(rev)) => mc / rev,
        _ => 10.0,
    };
    let mut score: f64 = 50.0;
    if pe_like < 15.0 {
        score += 18.0;
    } else if pe_like < 25.0 {
        score += 10.0;
    } else if pe_like > 60.0 {
        score -= 15.0;
    }
    if ps_like < 2.0 {
        score += 12.0;
    } else if ps_like > 8.0 {
        score -= 10.0;
    }
    score.clamp(0.0, 100.0)
}

fn profitability_score(item: &EnrichedCandidate) -> f64 {
    let Some(f) = item.fundamentals.as_ref() else {
        return 40.0;
    };
    let margin: f64 = match (
        f.net_income_usd,
        f.revenues_usd.filter(|value| *value > 0.0),
    ) {
        (Some(ni), Some(rev)) => ni / rev,
        _ => 0.0,
    };
    let cash_conversion: f64 = match (
        f.operating_cash_flow_usd,
        f.net_income_usd.filter(|value| value.abs() > 0.0),
    ) {
        (Some(ocf), Some(ni)) => ocf / ni,
        _ => 0.0,
    };
    (48.0 + margin.clamp(-0.2, 0.3) * 90.0 + cash_conversion.clamp(-1.0, 2.0) * 8.0)
        .clamp(0.0, 100.0)
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
    (75.0 - avg_abs_change.clamp(0.0, 18.0) * 2.0 - latest_turnover.clamp(0.0, 40.0) * 0.5)
        .clamp(0.0, 100.0)
}

fn event_score(item: &EnrichedCandidate) -> f64 {
    let disclosure_count = item
        .news_snapshot
        .deep_item_count
        .max(item.news_snapshot.light_item_count) as f64;
    let recency_support = (!item.news_snapshot.latest_published_at.trim().is_empty()) as i32 as f64;
    let catalyst_support = item.news_snapshot.catalyst_count.min(4) as f64;
    (35.0 + disclosure_count.min(8.0) * 4.0 + recency_support * 8.0 + catalyst_support * 6.0)
        .clamp(0.0, 100.0)
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

pub fn penalty_score(item: &EnrichedCandidate) -> f64 {
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

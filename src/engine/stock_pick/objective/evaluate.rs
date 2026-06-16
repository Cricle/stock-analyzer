use crate::i18n::I18n;
use crate::models::{
    LocalText, ScoreDimension,
    StockPickItem, StockPickObjectiveAssessment,
    StockPickObjectiveBreakdown,
};
use crate::engine::stock_pick::EnrichedCandidate;
use std::collections::HashSet;
use crate::engine::math_utils::{sigmoid, exponential_decay};

use super::{IndustryAverages, AdvancedMetrics};
use super::{stock_pick_objective_cap, stock_pick_objective_grade, stock_pick_objective_gaps, stock_pick_objective_headline};
use super::default_headline_key;
pub(crate) fn evaluate_stock_pick_objective_assessment(
    pick: &StockPickItem,
    item: &EnrichedCandidate,
    metrics: &AdvancedMetrics,
    industry_avg: &IndustryAverages,
    i18n: &I18n,
    lang: &str,
) -> StockPickObjectiveAssessment {
    let data_completeness = score_pick_data_completeness(pick, item);
    let market_validation = score_pick_market_validation(pick, item);
    let reasoning_structure = score_pick_reasoning_structure(pick);
    let risk_balance = score_pick_risk_balance(pick, item);
    let evidence_density = score_pick_evidence_density(pick, item);

    // Normalize each dimension to 0-100, then weighted average
    let norm = |s: &ScoreDimension| -> f64 {
        if s.max_score > 0 { s.score as f64 / s.max_score as f64 * 100.0 } else { 0.0 }
    };
    let weights = [0.15, 0.25, 0.20, 0.20, 0.20]; // data, market, reasoning, risk, evidence
    let scores = [
        norm(&data_completeness),
        norm(&market_validation),
        norm(&reasoning_structure),
        norm(&risk_balance),
        norm(&evidence_density),
    ];
    let weighted: f64 = scores.iter().zip(weights.iter()).map(|(s, w)| s * w).sum();
    let applied_cap = stock_pick_objective_cap(pick, item, metrics, industry_avg);
    let final_score = (weighted as i32).clamp(0, applied_cap);
    let grade = stock_pick_objective_grade(final_score);
    let gaps = stock_pick_objective_gaps(pick, item, industry_avg);
    let ready = final_score >= 75 && gaps.len() <= 2;
    let headline = stock_pick_objective_headline(final_score, ready, &gaps, i18n, lang);

    StockPickObjectiveAssessment {
        final_score,
        grade: grade.to_string(),
        ready,
        headline,
        headline_key: Some(default_headline_key(final_score, ready, &gaps, i18n, lang)),
        gaps,
        breakdown: StockPickObjectiveBreakdown {
            data_completeness,
            market_validation,
            reasoning_structure,
            risk_balance,
            evidence_density,
            total_score: final_score,
        },
    }
}

fn score_pick_data_completeness(pick: &StockPickItem, item: &EnrichedCandidate) -> ScoreDimension {
    let mut score = 0;
    let mut covered = Vec::new();
    if pick.price.is_some() {
        score += 2;
        covered.push("price");
    }
    if pick.change_pct.is_some() {
        score += 2;
        covered.push("change_pct");
    }
    if pick.market_cap.is_some() {
        score += 3;
        covered.push("market_cap");
    }
    if item.fundamentals.is_some() {
        score += 3;
        covered.push("fundamentals");
    }
    if item
        .fundamentals
        .as_ref()
        .and_then(|value| value.shares_outstanding)
        .is_some()
    {
        score += 3;
        covered.push("shares_outstanding");
    }
    if item
        .fundamentals
        .as_ref()
        .and_then(|value| value.industry.as_ref())
        .is_some_and(|value| !value.trim().is_empty() && value != "Unknown")
    {
        score += 3;
        covered.push("industry");
    }
    if item
        .fundamentals
        .as_ref()
        .is_some_and(|value| value.revenues_usd.is_some() || value.net_income_usd.is_some())
    {
        score += 4;
        covered.push("income_statement");
    }
    if item.fundamentals.as_ref().is_some_and(|value| {
        value.assets_usd.is_some()
            || value.liabilities_usd.is_some()
            || value.stockholders_equity_usd.is_some()
            || value.cash_and_equivalents_usd.is_some()
    }) {
        score += 5;
        covered.push("balance_sheet");
    }
    // Enrichment data bonus
    if item.enrichment.pe_ttm.is_some() || item.enrichment.pb.is_some() {
        score += 2;
        covered.push("valuation_enrichment");
    }
    if item.enrichment.revenue_yoy.is_some() || item.enrichment.net_profit_yoy.is_some() {
        score += 2;
        covered.push("earnings_growth");
    }
    if item.enrichment.fund_flow_net_ratio.is_some() {
        score += 1;
        covered.push("fund_flow");
    }
    if item.enrichment.analyst_report_count.is_some() {
        score += 1;
        covered.push("analyst_coverage");
    }
    if item.enrichment.chip_benefit_ratio.is_some() {
        score += 1;
        covered.push("chip_distribution");
    }
    if item.enrichment.dividend_yield.is_some() {
        score += 1;
        covered.push("dividend");
    }
    ScoreDimension {
        score,
        max_score: 33,
        rationale: LocalText::new("pick_data_completeness_rationale")
            .with_str("covered_fields", covered.join(", ")),
    }
}

fn score_pick_market_validation(pick: &StockPickItem, item: &EnrichedCandidate) -> ScoreDimension {
    let mut score = 0.0_f64;
    let mut reasons = Vec::new();
    let candle_count = item.candles.len();

    // 1. Candle count — sigmoid (0-4 points)
    let candle_score = sigmoid(candle_count as f64, 15.0, 0.12) * 4.0;
    score += candle_score;
    if candle_count >= 20 {
        reasons.push(">=20 candles");
    } else if candle_count >= 10 {
        reasons.push(">=10 candles");
    } else if candle_count >= 5 {
        reasons.push(">=5 candles");
    }

    // Basic data presence (binary checks — these are factual)
    if pick.change_pct.is_some_and(|value| value.is_finite()) {
        score += 2.0;
        reasons.push("valid daily change");
    }
    if pick.price.is_some_and(|value| value > 0.0) {
        score += 2.0;
        reasons.push("positive last price");
    }

    // 2. News count — sigmoid (1-3 points, base 1 for having any data source)
    let news_count = item.news.len();
    let news_score = 1.0 + sigmoid(news_count as f64, 2.0, 1.0) * 2.0;
    score += news_score;
    if news_count >= 5 {
        reasons.push(">=5 news items");
    } else if news_count >= 3 {
        reasons.push(">=3 news items");
    } else if news_count >= 1 {
        reasons.push(">=1 news item");
    }

    // Computed candle deltas (binary)
    if item.candles.iter().skip(1).any(|row| row.change_pct.is_finite()) {
        score += 1.0;
        reasons.push("computed candle deltas");
    }
    if pick.change_pct.is_some_and(|value| value.abs() <= 20.0) {
        score += 1.0;
        reasons.push("plausible daily move");
    }

    // 3. Trend ratio — sigmoid (0-3 points)
    let up_days = item
        .candles
        .windows(2)
        .filter(|window| window[1].close >= window[0].close)
        .count();
    let trend_ratio = if candle_count > 1 {
        up_days as f64 / (candle_count - 1) as f64
    } else {
        0.0
    };
    let trend_score = sigmoid(trend_ratio, 0.55, 15.0) * 3.0;
    score += trend_score;
    if trend_ratio >= 0.65 {
        reasons.push("high up-day ratio");
    } else if trend_ratio >= 0.55 {
        reasons.push("moderate up-day ratio");
    }

    // 4. Trailing drawdown — exponential decay (0-4 points)
    let latest_close = item.candles.last().map(|row| row.close).unwrap_or_default();
    let rolling_high = item
        .candles
        .iter()
        .map(|row| row.close)
        .fold(0.0_f64, f64::max);
    let trailing_drawdown_pct = if rolling_high > 0.0 {
        ((rolling_high - latest_close) / rolling_high) * 100.0
    } else {
        100.0
    };
    let drawdown_score = exponential_decay(trailing_drawdown_pct, 12.0) * 4.0;
    score += drawdown_score;
    if trailing_drawdown_pct <= 5.0 {
        reasons.push("tight drawdown");
    } else if trailing_drawdown_pct <= 10.0 {
        reasons.push("controlled drawdown");
    }

    // 5. Volatility — sigmoid inverse (0-2 points, lower is better)
    let avg_abs_change = if candle_count > 0 {
        item.candles
            .iter()
            .map(|row| row.change_pct.abs())
            .sum::<f64>()
            / candle_count as f64
    } else {
        0.0
    };
    let vol_score = (1.0 - sigmoid(avg_abs_change, 3.0, 1.0)) * 2.0;
    score += vol_score;
    if avg_abs_change <= 2.5 {
        reasons.push("contained volatility");
    } else if avg_abs_change <= 4.5 {
        reasons.push("moderate volatility");
    }

    // 6. Recent 5-day strength — sigmoid (±3 points)
    let recent_window: Vec<_> = item.candles.iter().rev().take(5).cloned().collect();
    let recent_return_pct = recent_window
        .first()
        .zip(recent_window.last())
        .and_then(|(latest, earliest)| {
            (earliest.close > 0.0).then_some(((latest.close / earliest.close) - 1.0) * 100.0)
        })
        .unwrap_or_default();
    let recent_score = sigmoid(recent_return_pct, -1.0, 0.4) * 4.0; // 0-4, generous center
    score += recent_score;
    if recent_return_pct > 1.0 {
        reasons.push("recent 5-day strength");
    } else if recent_return_pct < -1.0 {
        reasons.push("recent 5-day weakness");
    }

    // 7. Composite factor — sigmoid (0-2 points)
    let factor_score = sigmoid(item.factor.total, 55.0, 0.08) * 2.0;
    score += factor_score;
    if item.factor.total >= 65.0 {
        reasons.push("strong composite factor");
    } else if item.factor.total >= 55.0 {
        reasons.push("acceptable composite factor");
    }

    // 8. Enrichment data validation (0-4 points)
    let mut enrichment_pts = 0.0_f64;
    if item.fundamental_snapshot.pe_ttm.is_some_and(|v| v > 0.0) {
        enrichment_pts += 1.0;
        reasons.push("PE TTM available");
    }
    if item.fundamental_snapshot.revenue_yoy.is_some() {
        enrichment_pts += 1.0;
        reasons.push("earnings growth data");
    }
    if item.fundamental_snapshot.fund_flow_net_ratio.is_some() {
        enrichment_pts += 0.5;
        reasons.push("fund flow data");
    }
    if item.fundamental_snapshot.analyst_report_count.is_some_and(|v| v > 0) {
        enrichment_pts += 0.5;
        reasons.push("analyst coverage");
    }
    // Analyst consensus validation
    if let Some(buy_ratio) = item.fundamental_snapshot.analyst_buy_ratio {
        if buy_ratio >= 0.6 {
            enrichment_pts += 1.0;
            reasons.push("strong analyst consensus");
        } else if buy_ratio >= 0.4 {
            enrichment_pts += 0.5;
            reasons.push("moderate analyst consensus");
        }
    }
    score += enrichment_pts.min(4.0);

    ScoreDimension {
        score: score.clamp(0.0, 29.0) as i32,
        max_score: 29,
        rationale: LocalText::new("pick_market_validation_rationale")
            .with_str("validation_details", reasons.join(", ")),
    }
}

fn score_pick_reasoning_structure(pick: &StockPickItem) -> ScoreDimension {
    let mut score = 0;
    let mut reasons = Vec::new();
    let thesis_len = pick.thesis.trim().chars().count();
    if thesis_len >= 80 {
        score += 4;
        reasons.push("thesis>=80 chars");
    } else if thesis_len >= 30 {
        score += 2;
        reasons.push("thesis>=30 chars");
    }
    if thesis_len >= 160 {
        score += 3;
        reasons.push("thesis>=160 chars");
    }
    if pick.catalysts.len() >= 2 {
        score += 4;
        reasons.push(">=2 catalysts");
    } else if !pick.catalysts.is_empty() {
        score += 2;
        reasons.push("1 catalyst");
    }
    if pick.risks.len() >= 2 {
        score += 4;
        reasons.push(">=2 risks");
    } else if !pick.risks.is_empty() {
        score += 2;
        reasons.push("1 risk");
    }
    if pick.evidence_points.len() >= 4 {
        score += 5;
        reasons.push(">=4 evidence points");
    } else if !pick.evidence_points.is_empty() {
        score += 3;
        reasons.push(">=1 evidence point");
    }
    let unique_supports = pick
        .catalysts
        .iter()
        .chain(pick.risks.iter())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>()
        .len();
    if unique_supports >= 4 {
        score += 4;
        reasons.push("unique support/risk items");
    }
    ScoreDimension {
        score: score.min(25),
        max_score: 25,
        rationale: LocalText::new("pick_reasoning_structure_rationale")
            .with_str("structure_details", reasons.join(", ")),
    }
}

fn score_pick_risk_balance(pick: &StockPickItem, item: &EnrichedCandidate) -> ScoreDimension {
    let cat_count = pick.catalysts.len() as f64;
    let risk_count = pick.risks.len() as f64;
    let risk_factor = item.factor.risk;
    let total_items = cat_count + risk_count;
    let unique_catalysts = pick
        .catalysts
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>()
        .len();
    let unique_risks = pick
        .risks
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>()
        .len();

    let mut score = 0.0_f64;

    // 1. Total coverage — sigmoid (0-7 points, having more items = better)
    score += sigmoid(total_items, 3.0, 0.8) * 7.0;

    // 2. Catalyst count — sigmoid (0-4 points)
    score += sigmoid(cat_count, 1.5, 1.2) * 4.0;

    // 3. Risk count — sigmoid (0-4 points, more risks identified = better)
    score += sigmoid(risk_count, 2.0, 1.0) * 4.0;

    // 4. Confidence — bell curve around 65% (0-3 points)
    let conf = pick.confidence / 100.0;
    let conf_score = (1.0 - (conf - 0.65).abs() * 3.0).max(0.0) * 3.0;
    score += conf_score;

    // 5. Uniqueness bonus (0-4 points)
    let unique_score = sigmoid(unique_catalysts as f64, 1.5, 1.2)
        * sigmoid(unique_risks as f64, 1.5, 1.2)
        * 4.0;
    score += unique_score;

    // 6. Risk factor awareness — sigmoid (0-3 points)
    score += sigmoid(risk_factor, 50.0, 0.06) * 3.0;

    // 7. Enrichment risk signals (±2 points)
    let mut enrichment_risk_adj = 0.0_f64;
    // Chip benefit: high ratio = less selloff pressure → bonus
    if let Some(chip) = item.fundamental_snapshot.chip_benefit_ratio {
        if chip >= 0.6 {
            enrichment_risk_adj += 1.0;
        } else if chip <= 0.3 {
            enrichment_risk_adj -= 0.5;
        }
    }
    // Fund flow: outflow = risk, inflow = support
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio {
        if flow < -0.05 {
            enrichment_risk_adj -= 1.0;
        } else if flow > 0.03 {
            enrichment_risk_adj += 0.5;
        }
    }
    // Valuation stretched (PE TTM > 80 = elevated risk)
    if let Some(pe_ttm) = item.fundamental_snapshot.pe_ttm
        && pe_ttm > 80.0
    {
        enrichment_risk_adj -= 0.5;
    }
    score = (score + enrichment_risk_adj).max(0.0);

    ScoreDimension {
        score: score.clamp(0.0, 27.0) as i32,
        max_score: 27,
        rationale: LocalText::new("pick_risk_balance_rationale")
            .with_i32("catalysts", pick.catalysts.len() as i32)
            .with_i32("risks", pick.risks.len() as i32)
            .with_i32("unique_catalysts", unique_catalysts as i32)
            .with_i32("unique_risks", unique_risks as i32)
            .with_f64("risk_factor", risk_factor),
    }
}

fn score_pick_evidence_density(pick: &StockPickItem, item: &EnrichedCandidate) -> ScoreDimension {
    let evidence_count = pick.evidence_points.len();
    let news_count = item.news.len();
    let candle_count = item.candles.len();

    let mut score = 0.0_f64;

    // 1. Evidence points — sigmoid (0-8 points)
    score += sigmoid(evidence_count as f64, 4.0, 1.0) * 8.0;

    // 2. News count — sigmoid (0-4 points)
    score += sigmoid(news_count as f64, 4.0, 0.8) * 4.0;

    // 3. Candle count — sigmoid (0-5 points)
    score += sigmoid(candle_count as f64, 15.0, 0.12) * 5.0;

    // 4. Financial data fields — sigmoid (0-5 points, more fields = better)
    let fin_fields = item
        .fundamentals
        .as_ref()
        .map(|f| {
            usize::from(f.revenues_usd.is_some())
                + usize::from(f.net_income_usd.is_some())
                + usize::from(f.assets_usd.is_some())
                + usize::from(f.liabilities_usd.is_some())
                + usize::from(f.stockholders_equity_usd.is_some())
                + usize::from(f.cash_and_equivalents_usd.is_some())
                + usize::from(f.gross_profit_usd.is_some())
                + usize::from(f.operating_cash_flow_usd.is_some())
        })
        .unwrap_or(0);
    score += sigmoid(fin_fields as f64, 4.0, 0.8) * 3.0;

    // 5. Enrichment data fields — sigmoid (0-4 points)
    let enrichment_fields = usize::from(item.enrichment.pe_ttm.is_some())
        + usize::from(item.enrichment.pb.is_some())
        + usize::from(item.enrichment.revenue_yoy.is_some())
        + usize::from(item.enrichment.net_profit_yoy.is_some())
        + usize::from(item.enrichment.fund_flow_net_ratio.is_some())
        + usize::from(item.enrichment.analyst_report_count.is_some())
        + usize::from(item.enrichment.chip_benefit_ratio.is_some())
        + usize::from(item.enrichment.dividend_yield.is_some());
    score += sigmoid(enrichment_fields as f64, 3.0, 0.8) * 4.0;

    ScoreDimension {
        score: score.clamp(0.0, 25.0) as i32,
        max_score: 25,
        rationale: LocalText::new("pick_evidence_density_rationale")
            .with_i32("evidence_count", evidence_count as i32)
            .with_i32("news_count", news_count as i32)
            .with_i32("candle_count", candle_count as i32),
    }
}

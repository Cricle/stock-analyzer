use std::collections::{HashMap, HashSet};

use crate::data::MarketKind;
use crate::{
    LocalText, ScoreDimension, StockPickItem, StockPickObjectiveAssessment,
    StockPickObjectiveBreakdown,
};

use crate::pick::EnrichedCandidate;

// ---------------------------------------------------------------------------
// criteria (inlined from objective/criteria.rs)
// ---------------------------------------------------------------------------

struct IndustryAverages {
    pe_avg: f64,
    ps_avg: f64,
}

fn compute_industry_averages(
    all_candidates: &[EnrichedCandidate],
) -> HashMap<String, IndustryAverages> {
    let mut pe_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut ps_sums: HashMap<String, Vec<f64>> = HashMap::new();

    for candidate in all_candidates {
        let industry = &candidate.industry;
        if industry == "Unknown" {
            continue;
        }
        if let Some(pe) = candidate.fundamental_snapshot.pe_like {
            pe_sums.entry(industry.clone()).or_default().push(pe);
        }
        if let Some(ps) = candidate.fundamental_snapshot.ps_like {
            ps_sums.entry(industry.clone()).or_default().push(ps);
        }
    }

    let mut averages = HashMap::new();
    let all_industries: HashSet<&String> = pe_sums.keys().chain(ps_sums.keys()).collect();

    for industry in all_industries {
        let pe_vals = pe_sums.get(industry);
        let ps_vals = ps_sums.get(industry);
        let count = pe_vals
            .map(|v| v.len())
            .unwrap_or(0)
            .max(ps_vals.map(|v| v.len()).unwrap_or(0));
        if count < 2 {
            continue;
        }
        let pe_avg = pe_vals.map(|v| v.iter().sum::<f64>() / v.len() as f64);
        let ps_avg = ps_vals.map(|v| v.iter().sum::<f64>() / v.len() as f64);
        if let (Some(pe), Some(ps)) = (pe_avg, ps_avg) {
            averages.insert(
                industry.clone(),
                IndustryAverages {
                    pe_avg: pe,
                    ps_avg: ps,
                },
            );
        }
    }
    averages
}

/// Compute Format_valuation_line.
pub fn format_valuation_line(label: &str, value: Option<f64>, avg: f64) -> Option<String> {
    let v = value?;
    if !v.is_finite() || v <= 0.0 {
        return None;
    }
    let premium = v / avg;
    let direction = if premium >= 1.0 {
        "premium"
    } else {
        "discount"
    };
    Some(format!(
        "{} {:.1}x vs industry avg {:.1}x ({:.1}x {})",
        label, v, avg, premium, direction
    ))
}

/// Compute Build_valuation_vs_industry_block.
pub fn build_valuation_vs_industry_block(
    all_candidates: &[EnrichedCandidate],
    selected: &[EnrichedCandidate],
) -> String {
    let averages = compute_industry_averages(all_candidates);
    if averages.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    for candidate in selected {
        let industry = &candidate.industry;
        let Some(avg) = averages.get(industry) else {
            continue;
        };
        let mut parts = Vec::new();
        if let Some(line) =
            format_valuation_line("PE", candidate.fundamental_snapshot.pe_like, avg.pe_avg)
        {
            parts.push(line);
        }
        if let Some(line) =
            format_valuation_line("PS", candidate.fundamental_snapshot.ps_like, avg.ps_avg)
        {
            parts.push(line);
        }
        if !parts.is_empty() {
            lines.push(format!(
                "{} ({}): {}",
                candidate.symbol,
                industry,
                parts.join(", ")
            ));
        }
    }

    if lines.is_empty() {
        return String::new();
    }
    format!("Valuation vs Industry:\n{}\n\n", lines.join("\n"))
}

/// Compute Evaluate_stock_pick_objective_assessment.
pub fn evaluate_stock_pick_objective_assessment(
    pick: &StockPickItem,
    item: &EnrichedCandidate,
) -> StockPickObjectiveAssessment {
    // Existing 5 dimensions (unchanged)
    let data_completeness = score_pick_data_completeness(pick, item);
    let market_validation = score_pick_market_validation(pick, item);
    let reasoning_structure = score_pick_reasoning_structure(pick);
    let risk_balance = score_pick_risk_balance(pick, item);
    let evidence_density = score_pick_evidence_density(pick, item);

    // NEW: 3 additional dimensions
    let data_provenance_score = crate::pick::provenance::score_provenance(&item.provenance);
    let data_provenance = ScoreDimension {
        score: data_provenance_score,
        max_score: 20,
        rationale: LocalText::new("pick_data_provenance_rationale")
            .with_i32("score", data_provenance_score),
    };

    let reasoning_report = crate::pick::reasoning::validate_reasoning_consistency(pick, item);
    let reasoning_consistency = ScoreDimension {
        score: reasoning_report.consistency_score,
        max_score: 20,
        rationale: LocalText::new("pick_reasoning_consistency_rationale")
            .with_i32("major_violations", reasoning_report.major_violations as i32)
            .with_i32("minor_violations", reasoning_report.minor_violations as i32),
    };

    let completeness_result = crate::pick::completeness::score_critical_field_completeness(pick);
    let critical_field_completeness = ScoreDimension {
        score: completeness_result.score,
        max_score: 20,
        rationale: LocalText::new("pick_critical_field_completeness_rationale")
            .with_str("missing", completeness_result.missing_fields.join(", ")),
    };

    // Calculate raw total (max 160)
    let raw_total = [
        data_completeness.score,
        market_validation.score,
        reasoning_structure.score,
        risk_balance.score,
        evidence_density.score,
        data_provenance.score,
        reasoning_consistency.score,
        critical_field_completeness.score,
    ]
    .into_iter()
    .sum::<i32>();

    // Normalize to 100
    let normalized_score = ((raw_total as f64 / 160.0) * 100.0).round() as i32;

    // Apply existing cap logic
    let applied_cap = stock_pick_objective_cap(pick, item);
    let final_score = normalized_score.clamp(0, applied_cap);

    let grade = stock_pick_objective_grade(final_score);

    // Enhanced gaps with reasoning violations
    let mut gaps = stock_pick_objective_gaps(pick, item);
    for check in &reasoning_report.checks {
        if check.severity == "major" {
            gaps.push(format!("reasoning_violation: {}", check.claim));
        }
    }
    gaps.extend(
        completeness_result
            .missing_fields
            .iter()
            .map(|f| format!("missing_{}", f)),
    );

    // Ready criteria: enhanced with reasoning validation
    let has_market_cap = pick.market_cap.is_some()
        || item
            .fundamentals
            .as_ref()
            .and_then(|f| f.market_cap)
            .is_some();
    let ready = final_score >= 80
        && gaps.len() <= 2
        && has_market_cap
        && reasoning_report.major_violations == 0;

    let headline = stock_pick_objective_headline(final_score, ready, &gaps);

    StockPickObjectiveAssessment {
        final_score,
        grade: grade.to_string(),
        ready,
        headline,
        gaps,
        breakdown: StockPickObjectiveBreakdown {
            data_completeness,
            market_validation,
            reasoning_structure,
            risk_balance,
            evidence_density,
            data_provenance,
            reasoning_consistency,
            critical_field_completeness,
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
        score += 2;
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
        score += 2;
        covered.push("industry");
    }
    if item
        .fundamentals
        .as_ref()
        .is_some_and(|value| value.revenues_usd.is_some() || value.net_income_usd.is_some())
    {
        score += 3;
        covered.push("income_statement");
    }
    if item.fundamentals.as_ref().is_some_and(|value| {
        value.assets_usd.is_some()
            || value.liabilities_usd.is_some()
            || value.stockholders_equity_usd.is_some()
            || value.cash_and_equivalents_usd.is_some()
    }) {
        score += 3;
        covered.push("balance_sheet");
    }
    ScoreDimension {
        score,
        max_score: 20,
        rationale: LocalText::new("pick_data_completeness_rationale")
            .with_str("covered_fields", covered.join(", ")),
    }
}

fn score_pick_market_validation(pick: &StockPickItem, item: &EnrichedCandidate) -> ScoreDimension {
    let mut score = 0;
    let mut reasons = Vec::new();
    let candle_count = item.candles.len();
    let recent_window = item
        .candles
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    let recent_negative_days = recent_window
        .iter()
        .filter(|row| row.change_pct < 0.0)
        .count();
    let recent_positive_days = recent_window
        .iter()
        .filter(|row| row.change_pct > 0.0)
        .count();
    let recent_return_pct = recent_window
        .first()
        .zip(recent_window.last())
        .and_then(|(latest, earliest)| {
            (earliest.close > 0.0).then_some(((latest.close / earliest.close) - 1.0) * 100.0)
        })
        .unwrap_or_default();
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
    let avg_abs_change = if candle_count > 0 {
        item.candles
            .iter()
            .map(|row| row.change_pct.abs())
            .sum::<f64>()
            / candle_count as f64
    } else {
        0.0
    };
    if candle_count >= 20 {
        score += 4;
        reasons.push(">=20 candles");
    } else if candle_count >= 10 {
        score += 3;
        reasons.push(">=10 candles");
    } else if candle_count >= 5 {
        score += 2;
        reasons.push(">=5 candles");
    }
    if pick.change_pct.is_some_and(|value| value.is_finite()) {
        score += 1;
        reasons.push("valid daily change");
    }
    if pick.price.is_some_and(|value| value > 0.0) {
        score += 1;
        reasons.push("positive last price");
    }
    let news_count = item.news.len();
    if news_count >= 5 {
        score += 3;
        reasons.push(">=5 news items");
    } else if news_count >= 3 {
        score += 2;
        reasons.push(">=3 news items");
    } else if news_count >= 1 {
        score += 1;
        reasons.push(">=1 news item");
    }
    if item
        .candles
        .iter()
        .skip(1)
        .any(|row| row.change_pct.is_finite())
    {
        score += 1;
        reasons.push("computed candle deltas");
    }
    if pick.change_pct.is_some_and(|value| value.abs() <= 20.0) {
        score += 1;
        reasons.push("plausible daily move");
    }
    if trend_ratio >= 0.65 {
        score += 3;
        reasons.push("high up-day ratio");
    } else if trend_ratio >= 0.55 {
        score += 1;
        reasons.push("moderate up-day ratio");
    }
    if trailing_drawdown_pct <= 4.0 {
        score += 4;
        reasons.push("tight drawdown");
    } else if trailing_drawdown_pct <= 8.0 {
        score += 1;
        reasons.push("controlled drawdown");
    }
    if avg_abs_change <= 2.5 {
        score += 2;
        reasons.push("contained volatility");
    } else if avg_abs_change <= 4.5 {
        score += 1;
        reasons.push("moderate volatility");
    }
    if recent_positive_days >= 4 && recent_return_pct > 1.0 {
        score += 3;
        reasons.push("recent 5-day strength");
    } else if recent_negative_days >= 4 && recent_return_pct < -1.0 {
        score -= 3;
        reasons.push("recent 5-day weakness");
    } else if recent_negative_days >= 3 {
        score -= 2;
        reasons.push("recent pullback pressure");
    }
    if item.factor.total >= 65.0 {
        score += 3;
        reasons.push("strong composite factor");
    } else if item.factor.total >= 55.0 {
        score += 2;
        reasons.push("acceptable composite factor");
    }
    if item.factor.momentum >= 65.0 {
        score += 2;
        reasons.push("trend confirmation");
    }
    ScoreDimension {
        score: score.clamp(0, 20),
        max_score: 20,
        rationale: LocalText::new("pick_market_validation_rationale")
            .with_str("validation_details", reasons.join(", ")),
    }
}

fn score_pick_reasoning_structure(pick: &StockPickItem) -> ScoreDimension {
    let mut score = 0;
    let mut reasons = Vec::new();
    let thesis_len = pick.thesis.key.trim().chars().count();
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
        .map(|value| value.key.trim())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>()
        .len();
    if unique_supports >= 4 {
        score += 4;
        reasons.push("unique support/risk items");
    }
    ScoreDimension {
        score: score.min(20),
        max_score: 20,
        rationale: LocalText::new("pick_reasoning_structure_rationale")
            .with_str("structure_details", reasons.join(", ")),
    }
}

fn score_pick_risk_balance(pick: &StockPickItem, item: &EnrichedCandidate) -> ScoreDimension {
    let catalysts = pick.catalysts.len();
    let risks = pick.risks.len();
    let risk_factor = item.factor.risk;
    let mut score = 0;
    let unique_catalysts = pick
        .catalysts
        .iter()
        .map(|value| value.key.trim())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>()
        .len();
    let unique_risks = pick
        .risks
        .iter()
        .map(|value| value.key.trim())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>()
        .len();
    if catalysts > 0 && risks > 0 {
        score += 5;
    } else if catalysts > 0 || risks > 0 {
        score += 4;
    }
    if catalysts >= 2 && risks >= 2 {
        score += 3;
    }
    if catalysts > 0 && risks > 0 && (catalysts as i32 - risks as i32).abs() <= 1 {
        score += 3;
    }
    if (40.0..=90.0).contains(&pick.confidence) {
        score += 1;
    }
    if unique_catalysts >= 2 && unique_risks >= 2 {
        score += 2;
    }
    if risk_factor >= 60.0 && risks >= 2 {
        score += 2;
    } else if risk_factor < 40.0 && risks <= 1 {
        score -= 2;
    } else if risk_factor < 40.0 && risks >= 2 {
        score -= 1;
    }
    ScoreDimension {
        score: score.clamp(0, 20),
        max_score: 20,
        rationale: LocalText::new("pick_risk_balance_rationale")
            .with_i32("catalysts", catalysts as i32)
            .with_i32("risks", risks as i32)
            .with_i32("unique_catalysts", unique_catalysts as i32)
            .with_i32("unique_risks", unique_risks as i32)
            .with_f64("risk_factor", risk_factor),
    }
}

fn score_pick_evidence_density(pick: &StockPickItem, item: &EnrichedCandidate) -> ScoreDimension {
    let evidence_count = pick.evidence_points.len();
    let news_count = item.news.len();
    let candle_count = item.candles.len();
    let mut score = 0;
    if evidence_count >= 5 {
        score += 8;
    } else if evidence_count >= 3 {
        score += 6;
    } else if evidence_count > 0 {
        score += 3;
    }
    if news_count >= 5 {
        score += 4;
    } else if news_count >= 3 {
        score += 3;
    } else if news_count > 0 {
        score += 1;
    }
    if candle_count >= 20 {
        score += 5;
    } else if candle_count >= 10 {
        score += 4;
    } else if candle_count >= 5 {
        score += 3;
    }
    if item.fundamentals.as_ref().is_some_and(|value| {
        value.revenues_usd.is_some()
            || value.net_income_usd.is_some()
            || value.assets_usd.is_some()
            || value.liabilities_usd.is_some()
    }) {
        score += 3;
    }
    ScoreDimension {
        score,
        max_score: 20,
        rationale: LocalText::new("pick_evidence_density_rationale")
            .with_i32("evidence_count", evidence_count as i32)
            .with_i32("news_count", news_count as i32)
            .with_i32("candle_count", candle_count as i32),
    }
}

// ---------------------------------------------------------------------------
// scoring cap (inlined from objective/scoring/part2.rs)
// ---------------------------------------------------------------------------

fn stock_pick_objective_cap(pick: &StockPickItem, item: &EnrichedCandidate) -> i32 {
    let mut cap = 95;
    let fundamentals = item.fundamentals.as_ref();
    let market = normalize_market(&pick.market);
    let support_count = [
        item.factor.momentum,
        item.factor.quality,
        item.factor.value,
        item.factor.profitability,
        item.factor.risk,
        item.factor.event,
    ]
    .into_iter()
    .filter(|value| *value >= 55.0)
    .count();
    let has_industry = fundamentals
        .and_then(|value| value.industry.as_ref())
        .is_some_and(|value| !value.trim().is_empty() && value != "Unknown");
    let has_income_statement = fundamentals
        .is_some_and(|value| value.revenues_usd.is_some() || value.net_income_usd.is_some());
    let has_balance_sheet = fundamentals.is_some_and(|value| {
        value.assets_usd.is_some()
            || value.liabilities_usd.is_some()
            || value.stockholders_equity_usd.is_some()
            || value.cash_and_equivalents_usd.is_some()
    });

    if fundamentals.is_none() {
        cap = cap.min(72);
    }
    if fundamentals
        .is_some_and(|value| value.market_cap.is_none() || value.shares_outstanding.is_none())
    {
        cap = cap.min(75);
    }
    if !has_industry {
        cap = cap.min(88);
    }
    if !has_income_statement && !has_balance_sheet {
        cap = cap.min(84);
    }
    if item.news.len() < 3 {
        cap = cap.min(86);
    }
    if item.candles.len() < 10 {
        cap = cap.min(88);
    }
    if pick.evidence_points.len() < 4 {
        cap = cap.min(85);
    }
    if pick.catalysts.len() < 2 || pick.risks.len() < 2 {
        cap = cap.min(83);
    }
    if pick.thesis.key.trim().chars().count() < 120 {
        cap = cap.min(80);
    }
    if item.factor.total < 55.0 {
        cap = cap.min(79);
    }
    if item.factor.risk < 35.0 {
        cap = cap.min(81);
    }
    if support_count <= 2 {
        cap = cap.min(80);
    } else if support_count >= 4 && item.factor.total >= 60.0 {
        cap += 2;
    }
    if item.factor.momentum >= 70.0 && item.factor.event >= 60.0 {
        cap += 1;
    }
    if market == MarketKind::HongKong && (!has_income_statement || !has_balance_sheet) {
        cap = cap.min(78);
    }
    if market == MarketKind::AShare && !has_income_statement && !has_balance_sheet {
        cap = cap.min(82);
    }
    if market == MarketKind::UsEquity
        && (!has_income_statement || !has_balance_sheet || !has_industry)
    {
        cap = cap.min(90);
    }
    cap.clamp(60, 96)
}

// ---------------------------------------------------------------------------
// selection (inlined from objective/selection.rs)
// ---------------------------------------------------------------------------

/// Compute Stock_pick_objective_grade.
pub fn stock_pick_objective_grade(score: i32) -> &'static str {
    match score {
        85..=100 => "A",
        75..=84 => "B",
        60..=74 => "C",
        _ => "D",
    }
}

/// Compute Stock_pick_objective_gaps.
pub fn stock_pick_objective_gaps(pick: &StockPickItem, item: &EnrichedCandidate) -> Vec<String> {
    let mut gaps = Vec::new();
    let fundamentals = item.fundamentals.as_ref();
    if fundamentals.is_none() {
        gaps.push("missing_fundamentals".to_string());
        return gaps;
    }
    if fundamentals
        .and_then(|value| value.industry.as_ref())
        .is_none_or(|value| value.trim().is_empty() || value == "Unknown")
    {
        gaps.push("missing_industry".to_string());
    }
    if fundamentals
        .is_some_and(|value| value.revenues_usd.is_none() && value.net_income_usd.is_none())
    {
        gaps.push("missing_income_statement".to_string());
    }
    if fundamentals.is_some_and(|value| {
        value.assets_usd.is_none()
            && value.liabilities_usd.is_none()
            && value.stockholders_equity_usd.is_none()
    }) {
        gaps.push("missing_balance_sheet".to_string());
    }
    if item.news.len() < 3 {
        gaps.push("thin_news_coverage".to_string());
    }
    if item.candles.len() < 10 {
        gaps.push("short_price_history".to_string());
    }
    if pick.evidence_points.len() < 4 {
        gaps.push("thin_evidence_points".to_string());
    }
    gaps
}

/// Compute Stock_pick_objective_headline.
pub fn stock_pick_objective_headline(score: i32, ready: bool, gaps: &[String]) -> String {
    if ready && score >= 85 {
        return "High-quality candidate with broad evidence coverage.".to_string();
    }
    if ready {
        return "Usable candidate with acceptable evidence depth.".to_string();
    }
    if gaps.is_empty() {
        return "Evidence is mixed and still needs confirmation.".to_string();
    }
    format!("Not fully ready: {}.", gaps.join(", "))
}

fn normalize_market(value: &str) -> MarketKind {
    match value.trim().to_lowercase().as_str() {
        "a" | "a-share" | "a_share" | "ashare" | "cn" | "china" | "a股" => MarketKind::AShare,
        "hk" | "hkex" | "hongkong" | "hong_kong" | "港股" => MarketKind::HongKong,
        _ => MarketKind::UsEquity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pick::provenance::ProvenanceSnapshot;
    use crate::pick::types::{EnrichedCandidate, FactorBreakdown};
    use crate::{
        StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
        StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
        StockPickTechnicalSnapshot,
    };

    fn create_full_test_pick() -> StockPickItem {
        StockPickItem {
            symbol: "TEST".to_string(),
            name: "Test Stock".to_string(),
            market: "US".to_string(),
            thesis: crate::guide::I18nText::new("Strong technical setup with RSI support"),
            catalysts: vec![
                crate::guide::I18nText::new("Earnings report next week"),
                crate::guide::I18nText::new("Product launch"),
            ],
            risks: vec![
                crate::guide::I18nText::new("Market volatility"),
                crate::guide::I18nText::new("Regulatory concerns"),
            ],
            evidence_points: vec![
                "RSI at 35".to_string(),
                "MACD golden cross".to_string(),
                "Volume surge".to_string(),
                "Positive news sentiment".to_string(),
            ],
            confidence: 75.0,
            price: Some(100.0),
            change_pct: Some(2.0),
            market_cap: Some(1_000_000_000.0),
            entry_price: Some("100.00".to_string()),
            entry_rationale: Some(
                "Entry at 100 based on RSI support and breakout confirmation".to_string(),
            ),
            stop_loss: Some("95.00".to_string()),
            stop_rationale: Some("Stop loss at 95 based on recent swing low".to_string()),
            target_price: Some("120.00".to_string()),
            target_rationale: Some("Target price 120 based on measured move".to_string()),
            holding_period: Some("3-6 months".to_string()),
            exit_triggers: vec!["Break below support".to_string()],
            ..Default::default()
        }
    }

    fn create_full_test_candidate() -> EnrichedCandidate {
        EnrichedCandidate {
            symbol: "TEST".to_string(),
            name: "Test Stock".to_string(),
            market: "US".to_string(),
            exchange: "NASDAQ".to_string(),
            industry: "Technology".to_string(),
            price: Some(100.0),
            change_pct: Some(2.0),
            market_cap: Some(1_000_000_000.0),
            theme_key: "test".to_string(),
            fundamentals: None,
            analyst_consensus: None,
            news: vec![],
            evidence_records: vec![],
            candles: vec![],
            technical_snapshot: StockPickTechnicalSnapshot::default(),
            market_snapshot: StockPickMarketSnapshot::default(),
            fundamental_snapshot: StockPickFundamentalSnapshot::default(),
            news_snapshot: StockPickNewsSnapshot::default(),
            history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
            risk_snapshot: StockPickRiskSnapshot::default(),
            data_quality_snapshot: StockPickDataQualitySnapshot::default(),
            factor: FactorBreakdown::default(),
            pass_filter: true,
            rejected_reasons: vec![],
            description: "Test candidate".to_string(),
            provenance: ProvenanceSnapshot::default(),
        }
    }

    #[test]
    fn test_evaluate_assessment_8_dimensions() {
        let pick = create_full_test_pick();
        let candidate = create_full_test_candidate();

        let assessment = evaluate_stock_pick_objective_assessment(&pick, &candidate);

        // Verify all 8 dimensions present
        assert!(assessment.breakdown.data_completeness.score >= 0);
        assert!(assessment.breakdown.market_validation.score >= 0);
        assert!(assessment.breakdown.reasoning_structure.score >= 0);
        assert!(assessment.breakdown.risk_balance.score >= 0);
        assert!(assessment.breakdown.evidence_density.score >= 0);
        assert!(assessment.breakdown.data_provenance.score >= 0);
        assert!(assessment.breakdown.reasoning_consistency.score >= 0);
        assert!(assessment.breakdown.critical_field_completeness.score >= 0);

        // Total should be normalized to 0-100
        assert!(assessment.final_score >= 0 && assessment.final_score <= 100);
    }
}

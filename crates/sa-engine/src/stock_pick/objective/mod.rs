use std::collections::{HashMap, HashSet};

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sa_data::MarketKind;
#[cfg(test)]
use sa_data::{CandlePoint, NewsItem};
use sa_models::{
    LocalText, ScoreDimension, StockPickItem, StockPickObjectiveAssessment,
    StockPickObjectiveBreakdown, StockPickObjectiveBucket, StockPickObjectiveOverview,
};
#[cfg(test)]
use sa_models::{
    StockPickDataQualitySnapshot, StockPickFactorBreakdown, StockPickFundamentalSnapshot,
    StockPickHistoryMatchSnapshot, StockPickMarketSnapshot, StockPickNewsSnapshot,
    StockPickRiskSnapshot, StockPickTechnicalSnapshot,
};

use crate::stock_pick::EnrichedCandidate;

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

fn format_valuation_line(label: &str, value: Option<f64>, avg: f64) -> Option<String> {
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

fn build_valuation_vs_industry_block(
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

pub(crate) fn build_prompt(
    market: &str,
    strategy: &str,
    analysis_date: &str,
    language: &str,
    selected: &[EnrichedCandidate],
    all_candidates: &[EnrichedCandidate],
) -> String {
    let selected_block = selected
        .iter()
        .enumerate()
        .map(|(index, item)| {
            format!(
                "Candidate {}\nSymbol: {}\nName: {}\nFactor Total: {:.2}\nMarket Snapshot: price={:?}, change_pct={:?}, period_return_pct={:?}, volume_ratio={:?}\nTechnical Snapshot: rsi={:?}, macd_hist={:?}, ema10={:?}, sma50={:?}, sma200={:?}, atr={:?}, adx={:?}\nFundamental Snapshot: market_cap={:?}, pe_like={:?}, ps_like={:?}, roe={:?}, leverage={:?}\nNews Snapshot: deep_items={}, unique_sources={}, latest_published_at={}\nHistory Snapshot: samples={}, hit_rate={:?}, avg_alpha={:?}\nRisk Flags: {}\nData Gaps: {}\n",
                index + 1,
                item.symbol,
                item.name,
                item.factor.total,
                item.market_snapshot.current_price,
                item.market_snapshot.latest_change_pct,
                item.market_snapshot.period_return_pct,
                item.market_snapshot.volume_ratio,
                item.technical_snapshot.rsi,
                item.technical_snapshot.macd_hist,
                item.technical_snapshot.close_10_ema,
                item.technical_snapshot.close_50_sma,
                item.technical_snapshot.close_200_sma,
                item.technical_snapshot.atr,
                item.technical_snapshot.adx,
                item.fundamental_snapshot.market_cap,
                item.fundamental_snapshot.pe_like,
                item.fundamental_snapshot.ps_like,
                item.fundamental_snapshot.roe,
                item.fundamental_snapshot.leverage,
                item.news_snapshot.deep_item_count,
                item.news_snapshot.unique_source_count,
                item.news_snapshot.latest_published_at,
                item.history_match_snapshot.sample_count,
                item.history_match_snapshot.hit_rate,
                item.history_match_snapshot.average_alpha_return,
                if item.risk_snapshot.signal_codes.is_empty() {
                    "none".to_string()
                } else {
                    item.risk_snapshot.signal_codes.join(", ")
                },
                if item.data_quality_snapshot.gaps.is_empty() {
                    "none".to_string()
                } else {
                    item.data_quality_snapshot.gaps.join(", ")
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let rejected_block = all_candidates
        .iter()
        .filter(|item| !item.pass_filter)
        .map(|item| format!("{}: {}", item.symbol, item.rejected_reasons.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");

    let valuation_block = build_valuation_vs_industry_block(all_candidates, selected);
    // System ranking block: revealed only in Phase 3, after independent assessment
    let system_rank_block = selected
        .iter()
        .enumerate()
        .map(|(index, item)| {
            format!(
                "Rank {}: {} ({}) — System Score: {:.2}",
                index + 1,
                item.symbol,
                item.name,
                item.factor.total,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are a senior equity selector.\n\
         Return strict JSON only with no markdown fences.\n\n\
         Market: {market}\n\
         Analysis Date: {analysis_date}\n\
         Strategy: {strategy}\n\
         Output language: {language}\n\n\
         ## Phase 1: Independent Evidence Review\n\
         Review the evidence below and form your OWN independent ranking.\n\
         Base your ranking solely on the evidence: technicals, fundamentals, news, risk flags, and data quality.\n\
         Do NOT assume the system ranking is correct — you may disagree.\n\n\
         Candidates:\n\
         {selected_block}\n\n\
         {valuation_block}\n\
         Filtered or rejected candidates:\n\
         {rejected_block}\n\n\
         ## Phase 2: Your Independent Picks\n\
         Select your top picks from the candidates above based purely on the evidence.\n\
         For each pick, write a substantive thesis grounded in specific data points.\n\
         If the evidence suggests a candidate is weaker than its position implies, lower its confidence or remove it.\n\
         If a rejected or lower-ranked candidate has strong evidence, consider promoting it.\n\n\
         ## Phase 3: Compare with System Ranking\n\
         The system ranking (by composite factor score) is:\n\
         {system_rank_block}\n\n\
         Compare your independent assessment with the system ranking:\n\
         - If you agree, set agreement_with_system_rank to \"agree\"\n\
         - If you would reorder some picks but keep mostly the same set, set it to \"partial\"\n\
         - If you fundamentally disagree, set it to \"disagree\"\n\
         For any difference, provide override_actions explaining WHY the evidence supports your alternative.\n\
         Disagreement is expected and healthy when evidence warrants it.\n\n\
         Required JSON schema:\n\
         {{{{\n\
           \"summary\": \"portfolio-level explanation\",\n\
           \"picks\": [\n\
             {{{{\n\
               \"symbol\": \"ticker\",\n\
               \"confidence\": 0-1,\n\
               \"thesis\": \"one paragraph thesis\",\n\
               \"catalysts\": [\"...\"],\n\
               \"risks\": [\"...\"],\n\
               \"evidence_points\": [\"...\"],\n\
               \"decision_reason_codes\": [\"score_leader\", \"technical_support\", \"fundamental_support\", \"evidence_support\", \"history_support\", \"risk_capped\"],\n\
               \"data_gaps\": [\"missing_history\", \"missing_fundamentals\"]\n\
             }}}}\n\
           ],\n\
           \"rejected_symbols\": [\"ticker\"],\n\
           \"agreement_with_system_rank\": \"agree|partial|disagree\",\n\
           \"override_actions\": [\n\
             {{{{\n\
               \"symbol\": \"ticker\",\n\
               \"action\": \"remove|raise|lower\",\n\
               \"reason_code\": \"evidence_conflict\",\n\
               \"rationale\": \"short rationale\"\n\
             }}}}\n\
           ]\n\
         }}}}",
    )
}

pub(crate) fn default_thesis(item: &EnrichedCandidate) -> String {
    format!(
        "{} The composite factor score is {:.1}，with momentum {:.1}、quality {:.1}、value {:.1}、profitability {:.1}、risk {:.1}、event {:.1}。It passed rule filters and was retained under sector diversification constraints, suitable as a balanced pick in the current candidate pool.",
        item.name,
        item.factor.total,
        item.factor.momentum,
        item.factor.quality,
        item.factor.value,
        item.factor.profitability,
        item.factor.risk,
        item.factor.event
    )
}

pub(crate) fn default_catalysts(item: &EnrichedCandidate) -> Vec<String> {
    let mut catalysts = Vec::new();
    if item.factor.momentum >= 70.0 {
        catalysts.push("Recent price trend and volume momentum are strong".to_string());
    }
    if item.factor.event >= 60.0 {
        catalysts.push("Recent announcements or news catalysts are relatively clear".to_string());
    }
    if item.factor.quality >= 60.0 {
        catalysts.push(
            "Quality factor is acceptable with reasonable balance sheet and earnings structure"
                .to_string(),
        );
    }
    if catalysts.is_empty() {
        catalysts.push("Composite factor score is relatively leading".to_string());
    }
    catalysts
}

pub(crate) fn default_risks(item: &EnrichedCandidate) -> Vec<String> {
    let mut risks = Vec::new();
    if item.change_pct.unwrap_or_default() >= 9.5 {
        risks.push("Short-term gain is large, increasing pullback risk from chasing".to_string());
    }
    if item.factor.value < 45.0 {
        risks.push("Valuation factor is average, cost-effectiveness not standout".to_string());
    }
    if item.factor.risk < 50.0 {
        risks.push("Volatility or turnover level is elevated".to_string());
    }
    if risks.is_empty() {
        risks.push(
            "Need to continue tracking price-volume and announcement fulfillment".to_string(),
        );
    }
    risks
}

pub(crate) fn evaluate_stock_pick_objective_assessment(
    pick: &StockPickItem,
    item: &EnrichedCandidate,
) -> StockPickObjectiveAssessment {
    let data_completeness = score_pick_data_completeness(pick, item);
    let market_validation = score_pick_market_validation(pick, item);
    let reasoning_structure = score_pick_reasoning_structure(pick);
    let risk_balance = score_pick_risk_balance(pick, item);
    let evidence_density = score_pick_evidence_density(pick, item);
    let total_score = [
        data_completeness.score,
        market_validation.score,
        reasoning_structure.score,
        risk_balance.score,
        evidence_density.score,
    ]
    .into_iter()
    .sum::<i32>();
    let applied_cap = stock_pick_objective_cap(pick, item);
    let final_score = total_score.clamp(0, applied_cap);
    let grade = stock_pick_objective_grade(final_score);
    let gaps = stock_pick_objective_gaps(pick, item);
    let ready = final_score >= 75 && gaps.len() <= 2;
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
            (earliest.close > Decimal::ZERO)
                .then_some(((latest.close / earliest.close) - Decimal::ONE) * Decimal::from(100))
        })
        .map(|v| v.to_f64().unwrap_or_default())
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
    let latest_close = item
        .candles
        .last()
        .map(|row| row.close.to_f64().unwrap_or_default())
        .unwrap_or_default();
    let rolling_high = item
        .candles
        .iter()
        .map(|row| row.close.to_f64().unwrap_or_default())
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
        cap = cap.min(82);
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
    if pick.thesis.trim().chars().count() < 120 {
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

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_objective_score_signal_separation(
    total: f64,
    momentum: f64,
    risk: f64,
    event: f64,
    with_industry: bool,
    with_financials: bool,
) -> i32 {
    use sa_data::FundamentalsSnapshot;

    let fundamentals = FundamentalsSnapshot {
        symbol: "T".to_string(),
        company_name: "Test".to_string(),
        cik: String::new(),
        industry: with_industry.then_some("Tech".to_string()),
        currency: "USD".to_string(),
        fiscal_year_end: None,
        shares_outstanding: Some(1_000_000_000),
        market_cap: Some(Decimal::from(100_000_000_000u64)),
        net_income_usd: with_financials.then_some(Decimal::from(10_000_000_000u64)),
        revenues_usd: with_financials.then_some(Decimal::from(50_000_000_000u64)),
        assets_usd: with_financials.then_some(Decimal::from(80_000_000_000u64)),
        liabilities_usd: with_financials.then_some(Decimal::from(20_000_000_000u64)),
        stockholders_equity_usd: with_financials.then_some(Decimal::from(60_000_000_000u64)),
        cash_and_equivalents_usd: with_financials.then_some(Decimal::from(12_000_000_000u64)),
        gross_profit_usd: None,
        operating_income_usd: None,
        operating_expenses_usd: None,
        operating_cash_flow_usd: None,
        capital_expenditure_usd: None,
        free_cash_flow_usd: None,
        long_term_debt_usd: None,
        current_debt_usd: None,
        total_debt_usd: None,
        diluted_shares_outstanding: Some(1_000_000_000),
    };
    let item = EnrichedCandidate {
        symbol: "T".to_string(),
        name: "Test".to_string(),
        market: "A-share".to_string(),
        exchange: "CN".to_string(),
        industry: if with_industry {
            "Tech".to_string()
        } else {
            "Unknown".to_string()
        },
        price: Some(10.0),
        change_pct: Some(1.5),
        market_cap: Some(100_000_000_000.0),
        theme_key: "test".to_string(),
        fundamentals: Some(fundamentals),
        news: vec![
            NewsItem {
                published_at: "2026-05-09".to_string(),
                title: "n1".to_string(),
                summary: "s1".to_string(),
                source: "x".to_string(),
                url: None,
            },
            NewsItem {
                published_at: "2026-05-08".to_string(),
                title: "n2".to_string(),
                summary: "s2".to_string(),
                source: "y".to_string(),
                url: None,
            },
            NewsItem {
                published_at: "2026-05-07".to_string(),
                title: "n3".to_string(),
                summary: "s3".to_string(),
                source: "z".to_string(),
                url: None,
            },
        ],
        candles: (0..12)
            .map(|index| CandlePoint {
                trade_date: format!("2026-05-{:02}", index + 1),
                open: Decimal::from(10),
                close: Decimal::from(10) + Decimal::from(index) / Decimal::from(10),
                high: Decimal::from(102) / Decimal::from(10)
                    + Decimal::from(index) / Decimal::from(10),
                low: Decimal::from(98) / Decimal::from(10)
                    + Decimal::from(index) / Decimal::from(10),
                volume: 1_000_000 + index as i64,
                amount: Decimal::ZERO,
                amplitude_pct: 1.0,
                change_pct: 1.0,
                change_amount: Decimal::from(1) / Decimal::from(10),
                turnover_pct: 1.0,
            })
            .collect(),
        factor: crate::stock_pick::FactorBreakdown {
            total,
            momentum,
            quality: 60.0,
            value: 55.0,
            profitability: 58.0,
            risk,
            event,
            evidence: 55.0,
            history: 50.0,
            penalty: 0.0,
        },
        pass_filter: true,
        rejected_reasons: Vec::new(),
        description: String::new(),
        evidence_records: Vec::new(),
        technical_snapshot: StockPickTechnicalSnapshot::default(),
        market_snapshot: StockPickMarketSnapshot::default(),
        fundamental_snapshot: StockPickFundamentalSnapshot::default(),
        news_snapshot: StockPickNewsSnapshot::default(),
        history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
        risk_snapshot: StockPickRiskSnapshot::default(),
        data_quality_snapshot: StockPickDataQualitySnapshot::default(),
    };
    let pick = StockPickItem {
        symbol: "T".to_string(),
        name: "Test".to_string(),
        market: "A-share".to_string(),
        exchange: "CN".to_string(),
        score: total,
        confidence: 74.0,
        thesis: "A sufficiently detailed thesis that is long enough to clear the structure threshold and explain the signal quality difference between candidates.".to_string(),
        catalysts: vec!["c1".to_string(), "c2".to_string()],
        risks: vec!["r1".to_string(), "r2".to_string()],
        evidence_points: vec!["e1".to_string(), "e2".to_string(), "e3".to_string(), "e4".to_string()],
        price: Some(10.0),
        change_pct: Some(1.5),
        market_cap: Some(100_000_000_000.0),
        priority_label: String::new(),
        priority_rank: 0,
        sort_key: 0.0,
        objective_assessment: StockPickObjectiveAssessment::default(),
        factor_breakdown: StockPickFactorBreakdown::default(),
        market_snapshot: StockPickMarketSnapshot::default(),
        technical_snapshot: StockPickTechnicalSnapshot::default(),
        fundamental_snapshot: StockPickFundamentalSnapshot::default(),
        news_snapshot: StockPickNewsSnapshot::default(),
        history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
        risk_snapshot: StockPickRiskSnapshot::default(),
        data_quality_snapshot: StockPickDataQualitySnapshot::default(),
        selection_reason_codes: Vec::new(),
        rejection_risk_flags: Vec::new(),
        evidence_quality_score: 0,
    };
    evaluate_stock_pick_objective_assessment(&pick, &item).final_score
}

pub(crate) fn summarize_stock_pick_objective_overview(
    picks: &[StockPickItem],
) -> StockPickObjectiveOverview {
    if picks.is_empty() {
        return StockPickObjectiveOverview::default();
    }
    let scores = picks
        .iter()
        .map(|item| item.objective_assessment.final_score)
        .collect::<Vec<_>>();
    let total = scores.iter().sum::<i32>() as f64;
    let average_score = total / scores.len() as f64;
    let buckets = [
        (
            "A",
            picks
                .iter()
                .filter(|item| item.objective_assessment.grade == "A")
                .count(),
        ),
        (
            "B",
            picks
                .iter()
                .filter(|item| item.objective_assessment.grade == "B")
                .count(),
        ),
        (
            "C",
            picks
                .iter()
                .filter(|item| item.objective_assessment.grade == "C")
                .count(),
        ),
        (
            "D",
            picks
                .iter()
                .filter(|item| item.objective_assessment.grade == "D")
                .count(),
        ),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(label, count)| StockPickObjectiveBucket {
        label: label.to_string(),
        count,
    })
    .collect::<Vec<_>>();
    StockPickObjectiveOverview {
        average_score,
        average_grade: stock_pick_objective_grade(average_score.round() as i32).to_string(),
        min_score: *scores.iter().min().unwrap_or(&0),
        max_score: *scores.iter().max().unwrap_or(&0),
        ready_picks: picks
            .iter()
            .filter(|item| item.objective_assessment.ready)
            .count(),
        incomplete_picks: picks
            .iter()
            .filter(|item| !item.objective_assessment.ready)
            .count(),
        distribution: buckets,
    }
}

fn stock_pick_objective_grade(score: i32) -> &'static str {
    match score {
        85..=100 => "A",
        75..=84 => "B",
        60..=74 => "C",
        _ => "D",
    }
}

fn stock_pick_objective_gaps(pick: &StockPickItem, item: &EnrichedCandidate) -> Vec<String> {
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

fn stock_pick_objective_headline(score: i32, ready: bool, gaps: &[String]) -> String {
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

pub(crate) fn stock_pick_priority_rank(pick: &StockPickItem) -> i32 {
    if pick.objective_assessment.ready && pick.objective_assessment.final_score >= 85 {
        1
    } else if pick.objective_assessment.ready && pick.objective_assessment.final_score >= 75 {
        2
    } else if pick.objective_assessment.final_score >= 65 {
        3
    } else {
        4
    }
}

pub(crate) fn stock_pick_priority_label(rank: i32) -> &'static str {
    match rank {
        1 => "high_priority",
        2 => "ready_watch",
        3 => "monitor",
        _ => "defer",
    }
}

pub(crate) fn stock_pick_sort_key(pick: &StockPickItem) -> f64 {
    let readiness_bonus = if pick.objective_assessment.ready {
        1000.0
    } else {
        0.0
    };
    readiness_bonus
        + ((5 - pick.priority_rank.clamp(1, 4)) as f64 * 100.0)
        + pick.objective_assessment.final_score as f64
        + pick.score
        + (pick.confidence / 10.0)
}

pub(crate) fn default_evidence(item: &EnrichedCandidate) -> Vec<String> {
    let mut evidence = Vec::new();
    if let (Some(first), Some(last)) = (item.candles.first(), item.candles.last()) {
        evidence.push(format!(
            "{} to {} closing price {:.2} -> {:.2}",
            first.trade_date, last.trade_date, first.close, last.close
        ));
        evidence.push(format!(
            "Daily change {:.2}%，volume {}",
            last.change_pct, last.volume
        ));
    }
    evidence.push(format!(
        "Composite factor score {:.1}，momentum {:.1}，quality {:.1}",
        item.factor.total, item.factor.momentum, item.factor.quality
    ));
    if !item.news.is_empty() {
        evidence.push(format!(
            "Recent news/announcement count {}",
            item.news.len()
        ));
    }
    evidence
}

fn normalize_market(value: &str) -> MarketKind {
    match value.trim().to_lowercase().as_str() {
        "a" | "a-share" | "a_share" | "ashare" | "cn" | "china" | "a股" => MarketKind::AShare,
        "hk" | "hkex" | "hongkong" | "hong_kong" | "港股" => MarketKind::HongKong,
        _ => MarketKind::UsEquity,
    }
}

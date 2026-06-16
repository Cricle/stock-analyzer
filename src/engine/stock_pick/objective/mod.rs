use std::collections::{HashMap, HashSet};

use crate::i18n::I18n;
use crate::models::{
    StockPickItem,
    StockPickObjectiveBucket, StockPickObjectiveOverview,
};
use crate::engine::stock_pick::EnrichedCandidate;
use crate::engine::math_utils::sigmoid;

mod advanced_metrics;
pub(crate) use advanced_metrics::AdvancedMetrics;

// ---------------------------------------------------------------------------
// criteria (inlined from objective/criteria.rs)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct IndustryAverages {
    pub(crate) pe_avg: f64,
    pub(crate) pe_std: f64,
    pub(crate) ps_avg: f64,
    pub(crate) ps_std: f64,
    pub(crate) roe_avg: f64,
    pub(crate) roe_std: f64,
    pub(crate) pe_ttm_avg: f64,
    pub(crate) pe_ttm_std: f64,
    pub(crate) pb_avg: f64,
    pub(crate) pb_std: f64,
    pub(crate) gross_margin_avg: f64,
    pub(crate) gross_margin_std: f64,
}

impl Default for IndustryAverages {
    fn default() -> Self {
        Self {
            pe_avg: 25.0,
            pe_std: 10.0,
            ps_avg: 5.0,
            ps_std: 3.0,
            roe_avg: 0.10,
            roe_std: 0.08,
            pe_ttm_avg: 25.0,
            pe_ttm_std: 10.0,
            pb_avg: 3.0,
            pb_std: 2.0,
            gross_margin_avg: 0.30,
            gross_margin_std: 0.15,
        }
    }
}

/// Look up industry averages with fallback to "default" for "Unknown" industries.
pub(crate) fn lookup_industry_avg<'a>(
    averages: &'a HashMap<String, IndustryAverages>,
    industry: &str,
) -> Option<&'a IndustryAverages> {
    averages.get(industry).or_else(|| {
        if industry == "Unknown" {
            averages.get("default")
        } else {
            None
        }
    })
}

pub(crate) fn compute_industry_averages(
    all_candidates: &[EnrichedCandidate],
) -> HashMap<String, IndustryAverages> {
    let mut pe_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut ps_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut roe_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut pe_ttm_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut pb_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut gm_sums: HashMap<String, Vec<f64>> = HashMap::new();

    // If all candidates have "Unknown" industry, group them under "default"
    // so cross-sectional z-scores can still be computed.
    let all_unknown = all_candidates
        .iter()
        .all(|c| c.industry == "Unknown");
    let default_key = "default".to_string();

    for candidate in all_candidates {
        let industry = if candidate.industry == "Unknown" && all_unknown {
            &default_key
        } else if candidate.industry == "Unknown" {
            continue;
        } else {
            &candidate.industry
        };
        if let Some(pe) = candidate.fundamental_snapshot.pe_like {
            pe_sums
                .entry(industry.clone())
                .or_default()
                .push(pe);
        }
        if let Some(ps) = candidate.fundamental_snapshot.ps_like {
            ps_sums
                .entry(industry.clone())
                .or_default()
                .push(ps);
        }
        if let Some(roe) = candidate.fundamental_snapshot.roe {
            roe_sums
                .entry(industry.clone())
                .or_default()
                .push(roe);
        }
        if let Some(pe_ttm) = candidate.fundamental_snapshot.pe_ttm.filter(|v| *v > 0.0) {
            pe_ttm_sums
                .entry(industry.clone())
                .or_default()
                .push(pe_ttm);
        }
        if let Some(pb) = candidate.fundamental_snapshot.pb.filter(|v| *v > 0.0) {
            pb_sums
                .entry(industry.clone())
                .or_default()
                .push(pb);
        }
        if let Some(gm) = candidate.fundamental_snapshot.gross_margin.filter(|v| *v > 0.0) {
            gm_sums
                .entry(industry.clone())
                .or_default()
                .push(gm);
        }
    }

    let mut averages = HashMap::new();
    let all_industries: HashSet<&String> = pe_sums
        .keys()
        .chain(ps_sums.keys())
        .chain(roe_sums.keys())
        .chain(pe_ttm_sums.keys())
        .chain(pb_sums.keys())
        .chain(gm_sums.keys())
        .collect();

    for industry in all_industries {
        let pe_vals = pe_sums.get(industry);
        let ps_vals = ps_sums.get(industry);
        let roe_vals = roe_sums.get(industry);
        let pe_ttm_vals = pe_ttm_sums.get(industry);
        let pb_vals = pb_sums.get(industry);
        let gm_vals = gm_sums.get(industry);
        let count = pe_vals.map(|v| v.len()).unwrap_or(0)
            .max(ps_vals.map(|v| v.len()).unwrap_or(0))
            .max(roe_vals.map(|v| v.len()).unwrap_or(0));
        if count < 2 {
            continue;
        }

        let (pe_avg, pe_std) = mean_std(pe_vals);
        let (ps_avg, ps_std) = mean_std(ps_vals);
        let (roe_avg, roe_std) = mean_std(roe_vals);
        let (pe_ttm_avg, pe_ttm_std) = mean_std(pe_ttm_vals);
        let (pb_avg, pb_std) = mean_std(pb_vals);
        let (gross_margin_avg, gross_margin_std) = mean_std(gm_vals);

        if pe_avg > 0.0 && ps_avg > 0.0 {
            averages.insert(
                industry.clone(),
                IndustryAverages {
                    pe_avg,
                    pe_std,
                    ps_avg,
                    ps_std,
                    roe_avg,
                    roe_std,
                    pe_ttm_avg,
                    pe_ttm_std,
                    pb_avg,
                    pb_std,
                    gross_margin_avg,
                    gross_margin_std,
                },
            );
        }
    }
    averages
}

/// Compute mean and standard deviation from optional slice.
fn mean_std(vals: Option<&Vec<f64>>) -> (f64, f64) {
    let Some(v) = vals else {
        return (0.0, 0.0);
    };
    if v.is_empty() {
        return (0.0, 0.0);
    }
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let variance = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64;
    (mean, variance.sqrt())
}

mod prompt;
mod evaluate;
pub(crate) use prompt::{build_prompt, default_thesis, default_catalyst_keys, default_risk_keys, default_thesis_key, default_evidence_keys, default_headline_key};
pub(crate) use evaluate::evaluate_stock_pick_objective_assessment;

// ---------------------------------------------------------------------------
// scoring cap (inlined from objective/scoring/part2.rs)
// ---------------------------------------------------------------------------

fn stock_pick_objective_cap(
    pick: &StockPickItem,
    item: &EnrichedCandidate,
    metrics: &AdvancedMetrics,
    _industry_avg: &IndustryAverages,
) -> i32 {
    let mut cap = 100.0_f64;
    let fundamentals = item.fundamentals.as_ref();
    let has_industry = fundamentals
        .and_then(|value| value.industry.as_ref())
        .is_some_and(|value| !value.trim().is_empty() && value != "Unknown");

    // Fundamentals data quality — single consolidated multiplier
    let fin_quality = match fundamentals {
        None => 0.85,
        Some(f) => {
            let fields = usize::from(f.revenues_usd.is_some())
                + usize::from(f.net_income_usd.is_some())
                + usize::from(f.assets_usd.is_some())
                + usize::from(f.stockholders_equity_usd.is_some())
                + usize::from(f.market_cap.is_some());
            0.92 + 0.08 * (fields as f64 / 5.0)
        }
    };
    cap *= fin_quality;

    // Industry bonus (not penalty)
    if has_industry {
        cap += 2.0;
    }

    // Factor quality — bonus for strong factors
    cap += sigmoid(item.factor.total, 55.0, 0.08) * 4.0;

    // Piotroski F-Score bonus
    if let Some(f_score) = metrics.piotroski_f_score {
        cap += (f_score as f64 / 7.0) * 3.0;
    }

    // ROIC bonus
    if let Some(roic) = metrics.roic {
        cap += sigmoid(roic, 0.10, 12.0) * 2.0;
    }

    // Enrichment z-score bonuses
    // PE TTM cheaper than industry → bonus
    if let Some(pe_ttm_z) = metrics.pe_ttm_deviation_z {
        cap += sigmoid(-pe_ttm_z, 0.5, 2.0) * 2.0; // negative z = cheaper = good
    }
    // PB cheaper than industry → bonus
    if let Some(pb_z) = metrics.pb_deviation_z {
        cap += sigmoid(-pb_z, 0.5, 2.0) * 1.5;
    }
    // Gross margin above industry → bonus
    if let Some(gm_z) = metrics.gross_margin_deviation_z {
        cap += sigmoid(gm_z, 0.5, 2.0) * 1.5;
    }

    // Evidence & thesis quality — small bonus
    cap += sigmoid(pick.evidence_points.len() as f64, 4.0, 1.0) * 2.0;
    let thesis_len = pick.thesis.trim().chars().count() as f64;
    cap += sigmoid(thesis_len, 100.0, 0.03) * 2.0;

    // Catalyst/risk balance bonus
    let cat_risk_min = pick.catalysts.len().min(pick.risks.len()) as f64;
    cap += sigmoid(cat_risk_min, 2.0, 1.5) * 2.0;

    cap.clamp(75.0, 100.0) as i32
}

// ---------------------------------------------------------------------------
// selection (inlined from objective/selection.rs)
// ---------------------------------------------------------------------------

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

pub(crate) fn stock_pick_objective_grade(score: i32) -> &'static str {
    match score {
        85..=100 => "A",
        75..=84 => "B",
        60..=74 => "C",
        _ => "D",
    }
}

pub(crate) fn stock_pick_objective_gaps(pick: &StockPickItem, item: &EnrichedCandidate, industry_avg: &IndustryAverages) -> Vec<String> {
    let mut gaps = Vec::new();
    let fundamentals = item.fundamentals.as_ref();
    if fundamentals.is_none() {
        gaps.push("missing_fundamentals".to_string());
        return gaps;
    }
    // Only flag missing_industry when no industry averages are available
    // (i.e., not even via the "default" fallback for Unknown industries)
    let has_industry = fundamentals
        .and_then(|value| value.industry.as_ref())
        .is_some_and(|value| !value.trim().is_empty() && value != "Unknown");
    let has_default_avg = industry_avg.pe_avg != 25.0 || industry_avg.ps_avg != 5.0; // non-default means averages were computed
    if !has_industry && !has_default_avg {
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
    // Enrichment gaps
    if item.enrichment.pe_ttm.is_none() && item.enrichment.pb.is_none() {
        gaps.push("missing_valuation_enrichment".to_string());
    }
    if item.enrichment.revenue_yoy.is_none() && item.enrichment.net_profit_yoy.is_none() {
        gaps.push("missing_earnings_growth".to_string());
    }
    gaps
}

pub(crate) fn stock_pick_objective_headline(score: i32, ready: bool, gaps: &[String], i18n: &I18n, lang: &str) -> String {
    if ready && score >= 85 {
        return i18n.resolve("stock_pick.headline.high_quality", lang)
            .unwrap_or_else(|| "High-quality candidate with broad evidence coverage.".to_string());
    }
    if ready {
        return i18n.resolve("stock_pick.headline.usable", lang)
            .unwrap_or_else(|| "Usable candidate with acceptable evidence depth.".to_string());
    }
    if gaps.is_empty() {
        return i18n.resolve("stock_pick.headline.mixed", lang)
            .unwrap_or_else(|| "Evidence is mixed and still needs confirmation.".to_string());
    }
    let translated_gaps: Vec<String> = gaps.iter().map(|g| {
        i18n.resolve(&format!("stock_pick.gap.{}", g), lang)
            .unwrap_or_else(|| g.clone())
    }).collect();
    let mut params = serde_json::Map::new();
    params.insert("gaps".to_string(), serde_json::Value::String(translated_gaps.join(", ")));
    i18n.resolve_with_params("stock_pick.headline.not_ready", lang, &params)
        .unwrap_or_else(|| format!("Not fully ready: {}.", translated_gaps.join(", ")))
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

pub(crate) fn default_evidence(item: &EnrichedCandidate, i18n: &I18n, lang: &str) -> Vec<String> {
    let mut evidence = Vec::new();
    let mut p;

    // Price data
    if let (Some(first), Some(last)) = (item.candles.first(), item.candles.last()) {
        if first.close > 0.0 {
            let ret = ((last.close / first.close) - 1.0) * 100.0;
            p = serde_json::Map::new();
            p.insert("start_date".to_string(), serde_json::Value::String(first.trade_date.clone()));
            p.insert("end_date".to_string(), serde_json::Value::String(last.trade_date.clone()));
            p.insert("start_price".to_string(), serde_json::json!(first.close));
            p.insert("end_price".to_string(), serde_json::json!(last.close));
            p.insert("ret".to_string(), serde_json::json!(ret));
            if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.price_range", lang, &p) {
                evidence.push(t);
            }
        }
        p = serde_json::Map::new();
        p.insert("change_pct".to_string(), serde_json::json!(last.change_pct));
        p.insert("volume".to_string(), serde_json::json!(last.volume));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.latest", lang, &p) {
            evidence.push(t);
        }
    }

    // Market cap
    if let Some(mc) = item.market_cap {
        if mc >= 1_000_000_000_000.0 {
            p = serde_json::Map::new();
            p.insert("cap".to_string(), serde_json::json!(mc / 1_000_000_000_000.0));
            if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.market_cap_t", lang, &p) {
                evidence.push(t);
            }
        } else if mc >= 100_000_000.0 {
            p = serde_json::Map::new();
            p.insert("cap".to_string(), serde_json::json!(mc / 100_000_000.0));
            if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.market_cap_b", lang, &p) {
                evidence.push(t);
            }
        }
    }

    // Fundamentals
    if let Some(pe) = item.fundamental_snapshot.pe_like {
        p = serde_json::Map::new();
        p.insert("pe".to_string(), serde_json::json!(pe));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.pe", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(roe) = item.fundamental_snapshot.roe {
        p = serde_json::Map::new();
        let roe_pct = roe * 100.0;
        if roe_pct.abs() > 100.0 {
            let annotation = i18n.resolve("stock_pick.evidence.negative_equity", lang).unwrap_or_default();
            p.insert("roe".to_string(), serde_json::json!(format!("{:.1}% ({annotation})", roe_pct)));
        } else {
            p.insert("roe".to_string(), serde_json::json!(roe_pct));
        }
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.roe", lang, &p) {
            evidence.push(t);
        }
    }

    // Enrichment data
    if let Some(pe_ttm) = item.fundamental_snapshot.pe_ttm {
        p = serde_json::Map::new();
        p.insert("pe_ttm".to_string(), serde_json::json!(pe_ttm));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.pe_ttm", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(pb) = item.fundamental_snapshot.pb {
        p = serde_json::Map::new();
        p.insert("pb".to_string(), serde_json::json!(pb));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.pb", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy {
        p = serde_json::Map::new();
        p.insert("pct".to_string(), serde_json::json!(rev_yoy * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.revenue_yoy", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy {
        p = serde_json::Map::new();
        p.insert("pct".to_string(), serde_json::json!(np_yoy * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.net_profit_yoy", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio {
        p = serde_json::Map::new();
        p.insert("pct".to_string(), serde_json::json!(flow * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.fund_flow", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(count) = item.fundamental_snapshot.analyst_report_count {
        p = serde_json::Map::new();
        p.insert("count".to_string(), serde_json::json!(count));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.analyst_reports", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(gm) = item.fundamental_snapshot.gross_margin {
        p = serde_json::Map::new();
        p.insert("pct".to_string(), serde_json::json!(gm * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.gross_margin", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(peg) = item.fundamental_snapshot.peg {
        p = serde_json::Map::new();
        p.insert("peg".to_string(), serde_json::json!(peg));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.peg", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(dy) = item.fundamental_snapshot.dividend_yield {
        p = serde_json::Map::new();
        p.insert("pct".to_string(), serde_json::json!(dy * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.dividend_yield", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(chip) = item.fundamental_snapshot.chip_benefit_ratio {
        p = serde_json::Map::new();
        p.insert("pct".to_string(), serde_json::json!(chip * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.chip_benefit", lang, &p) {
            evidence.push(t);
        }
    }

    // Technical
    if let Some(rsi) = item.technical_snapshot.rsi {
        p = serde_json::Map::new();
        p.insert("rsi".to_string(), serde_json::json!(rsi));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.rsi", lang, &p) {
            evidence.push(t);
        }
    }
    if let Some(macd_hist) = item.technical_snapshot.macd_hist {
        p = serde_json::Map::new();
        p.insert("macd".to_string(), serde_json::json!(macd_hist));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.macd_hist", lang, &p) {
            evidence.push(t);
        }
    }

    // Factor scores
    p = serde_json::Map::new();
    p.insert("total".to_string(), serde_json::json!(item.factor.total));
    p.insert("momentum".to_string(), serde_json::json!(item.factor.momentum));
    p.insert("quality".to_string(), serde_json::json!(item.factor.quality));
    p.insert("value".to_string(), serde_json::json!(item.factor.value));
    p.insert("growth".to_string(), serde_json::json!(item.factor.growth));
    if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.factor_summary", lang, &p) {
        evidence.push(t);
    }

    // Additional factor details
    p = serde_json::Map::new();
    p.insert("profitability".to_string(), serde_json::json!(item.factor.profitability));
    p.insert("risk".to_string(), serde_json::json!(item.factor.risk));
    p.insert("event".to_string(), serde_json::json!(item.factor.event));
    if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.factor_details", lang, &p) {
        evidence.push(t);
    }

    // News
    if !item.news.is_empty() {
        p = serde_json::Map::new();
        p.insert("count".to_string(), serde_json::json!(item.news.len()));
        p.insert("sources".to_string(), serde_json::json!(item.news_snapshot.unique_source_count));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.news_items", lang, &p) {
            evidence.push(t);
        }
    }

    // Candle stats
    if item.candles.len() >= 20 {
        let up_days = item.candles.windows(2)
            .filter(|w| w[1].close >= w[0].close)
            .count();
        let total = item.candles.windows(2).count().max(1);
        p = serde_json::Map::new();
        p.insert("count".to_string(), serde_json::json!(item.candles.len()));
        p.insert("ratio".to_string(), serde_json::json!(up_days as f64 / total as f64 * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.candle_stats", lang, &p) {
            evidence.push(t);
        }
    }

    // Industry
    if item.industry != "Unknown" {
        p = serde_json::Map::new();
        p.insert("industry".to_string(), serde_json::Value::String(item.industry.clone()));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.industry", lang, &p) {
            evidence.push(t);
        }
    }

    evidence
}


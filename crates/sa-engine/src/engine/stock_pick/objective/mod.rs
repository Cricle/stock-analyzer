use std::collections::{HashMap, HashSet};

use crate::i18n::I18n;
use crate::models::{
    LocalText, ScoreDimension,
    StockPickItem,
    StockPickObjectiveAssessment,
    StockPickObjectiveBreakdown, StockPickObjectiveBucket, StockPickObjectiveOverview,
};
use crate::engine::stock_pick::EnrichedCandidate;
use crate::engine::math_utils::{sigmoid, exponential_decay};

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

fn format_valuation_line(
    label: &str,
    value: Option<f64>,
    avg: f64,
    i18n: &I18n,
    lang: &str,
) -> Option<String> {
    let v = value?;
    if !v.is_finite() || v <= 0.0 {
        return None;
    }
    let premium = v / avg;
    let direction_key = if premium >= 1.0 {
        "stock_pick.valuation_vs_industry.premium"
    } else {
        "stock_pick.valuation_vs_industry.discount"
    };
    let direction = i18n.resolve(direction_key, lang).unwrap_or_else(|| direction_key.to_string());
    let mut params = serde_json::Map::new();
    params.insert("label".to_string(), serde_json::Value::String(label.to_string()));
    params.insert("value".to_string(), serde_json::json!(v));
    params.insert("avg".to_string(), serde_json::json!(avg));
    params.insert("premium".to_string(), serde_json::json!(premium));
    params.insert("direction".to_string(), serde_json::Value::String(direction));
    i18n.resolve_with_params("stock_pick.valuation_vs_industry.line", lang, &params)
        .or_else(|| Some(format!(
            "{} {:.1}x vs industry avg {:.1}x ({:.1}x {})",
            label, v, avg, premium,
            if premium >= 1.0 { "premium" } else { "discount" }
        )))
}

fn build_valuation_vs_industry_block(
    all_candidates: &[EnrichedCandidate],
    selected: &[EnrichedCandidate],
    i18n: &I18n,
    lang: &str,
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
        if let Some(line) = format_valuation_line(
            "PE",
            candidate.fundamental_snapshot.pe_like,
            avg.pe_avg,
            i18n,
            lang,
        ) {
            parts.push(line);
        }
        if let Some(line) = format_valuation_line(
            "PS",
            candidate.fundamental_snapshot.ps_like,
            avg.ps_avg,
            i18n,
            lang,
        ) {
            parts.push(line);
        }
        if let Some(line) = format_valuation_line(
            "PE_TTM",
            candidate.fundamental_snapshot.pe_ttm.filter(|v| *v > 0.0),
            avg.pe_ttm_avg,
            i18n,
            lang,
        ) {
            parts.push(line);
        }
        if let Some(line) = format_valuation_line(
            "PB",
            candidate.fundamental_snapshot.pb.filter(|v| *v > 0.0),
            avg.pb_avg,
            i18n,
            lang,
        ) {
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
    let header = i18n.resolve("stock_pick.valuation_vs_industry.header", lang)
        .unwrap_or_else(|| "Valuation vs Industry:\n".to_string());
    format!("{}{}\n\n", header, lines.join("\n"))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_prompt(
    market: &str,
    strategy: &str,
    analysis_date: &str,
    language: &str,
    selected: &[EnrichedCandidate],
    all_candidates: &[EnrichedCandidate],
    i18n: &I18n,
    lang: &str,
) -> String {
    let selected_block = selected
        .iter()
        .enumerate()
        .map(|(index, item)| {
            // Build enrichment line for prompt
            let mut enrich_parts = Vec::new();
            if let Some(pe_ttm) = item.fundamental_snapshot.pe_ttm {
                enrich_parts.push(format!("pe_ttm={:.1}", pe_ttm));
            }
            if let Some(pb) = item.fundamental_snapshot.pb {
                enrich_parts.push(format!("pb={:.1}", pb));
            }
            if let Some(peg) = item.fundamental_snapshot.peg {
                enrich_parts.push(format!("peg={:.2}", peg));
            }
            if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy {
                enrich_parts.push(format!("rev_yoy={:.1}%", rev_yoy * 100.0));
            }
            if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy {
                enrich_parts.push(format!("np_yoy={:.1}%", np_yoy * 100.0));
            }
            if let Some(gm) = item.fundamental_snapshot.gross_margin {
                enrich_parts.push(format!("gross_margin={:.1}%", gm * 100.0));
            }
            if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio {
                enrich_parts.push(format!("fund_flow={:.2}%", flow * 100.0));
            }
            if let Some(br) = item.fundamental_snapshot.analyst_buy_ratio {
                enrich_parts.push(format!("analyst_buy={:.0}%", br * 100.0));
            }
            if let Some(dy) = item.fundamental_snapshot.dividend_yield {
                enrich_parts.push(format!("div_yield={:.2}%", dy * 100.0));
            }
            if let Some(chip) = item.fundamental_snapshot.chip_benefit_ratio {
                enrich_parts.push(format!("chip_benefit={:.0}%", chip * 100.0));
            }
            let enrich_line = if enrich_parts.is_empty() {
                "none".to_string()
            } else {
                enrich_parts.join(", ")
            };

            format!(
                "Candidate {}\nSymbol: {}\nName: {}\nFactor Total: {:.2}\nMarket Snapshot: price={:?}, change_pct={:?}, period_return_pct={:?}, volume_ratio={:?}\nTechnical Snapshot: rsi={:?}, macd_hist={:?}, ema10={:?}, sma50={:?}, sma200={:?}, atr={:?}, adx={:?}\nFundamental Snapshot: market_cap={:?}, pe_like={:?}, ps_like={:?}, roe={:?}, leverage={:?}\nEnrichment: {}\nNews Snapshot: deep_items={}, unique_sources={}, latest_published_at={}\nHistory Snapshot: samples={}, hit_rate={:?}, avg_alpha={:?}\nRisk Flags: {}\nData Gaps: {}\n",
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
                enrich_line,
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

    let valuation_block = build_valuation_vs_industry_block(all_candidates, selected, i18n, lang);
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

pub(crate) fn default_thesis(item: &EnrichedCandidate, i18n: &I18n, lang: &str) -> String {
    let mut parts = Vec::new();

    // Price action with context
    if let (Some(first), Some(last)) = (item.candles.first(), item.candles.last()) {
        if first.close > 0.0 {
            let ret = ((last.close / first.close) - 1.0) * 100.0;
            let direction = if ret >= 5.0 {
                i18n.resolve("stock_pick.thesis.direction.strong_bullish", lang)
                    .unwrap_or_else(|| "strong bullish".to_string())
            } else if ret >= 0.0 {
                i18n.resolve("stock_pick.thesis.direction.moderate", lang)
                    .unwrap_or_else(|| "moderate".to_string())
            } else {
                i18n.resolve("stock_pick.thesis.direction.bearish", lang)
                    .unwrap_or_else(|| "bearish".to_string())
            };
            let mut params = serde_json::Map::new();
            params.insert("name".to_string(), serde_json::Value::String(item.name.clone()));
            params.insert("direction".to_string(), serde_json::Value::String(direction));
            params.insert("ret".to_string(), serde_json::json!(ret.abs()));
            params.insert("start_price".to_string(), serde_json::json!(first.close));
            params.insert("end_price".to_string(), serde_json::json!(last.close));
            if let Some(text) = i18n.resolve_with_params("stock_pick.thesis.price_action", lang, &params) {
                parts.push(text);
            }
        }
    } else {
        let mut params = serde_json::Map::new();
        params.insert("name".to_string(), serde_json::Value::String(item.name.clone()));
        params.insert("total".to_string(), serde_json::json!(item.factor.total));
        params.insert("momentum".to_string(), serde_json::json!(item.factor.momentum));
        params.insert("quality".to_string(), serde_json::json!(item.factor.quality));
        if let Some(text) = i18n.resolve_with_params("stock_pick.thesis.no_candles", lang, &params) {
            parts.push(text);
        }
    }

    // Valuation context
    let mut val_parts = Vec::new();
    if let Some(pe) = item.fundamental_snapshot.pe_like {
        let mut p = serde_json::Map::new();
        p.insert("pe".to_string(), serde_json::json!(pe));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.pe", lang, &p) {
            val_parts.push(t);
        }
    }
    if let Some(ps) = item.fundamental_snapshot.ps_like {
        let mut p = serde_json::Map::new();
        p.insert("ps".to_string(), serde_json::json!(ps));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.ps", lang, &p) {
            val_parts.push(t);
        }
    }
    if let Some(roe) = item.fundamental_snapshot.roe {
        let mut p = serde_json::Map::new();
        p.insert("roe".to_string(), serde_json::json!(roe * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.roe", lang, &p) {
            val_parts.push(t);
        }
    }
    if let Some(pb) = item.fundamental_snapshot.pb {
        let mut p = serde_json::Map::new();
        p.insert("pb".to_string(), serde_json::json!(pb));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.pb", lang, &p) {
            val_parts.push(t);
        }
    }
    if !val_parts.is_empty() {
        let mut p = serde_json::Map::new();
        p.insert("metrics".to_string(), serde_json::Value::String(val_parts.join(", ")));
        if let Some(text) = i18n.resolve_with_params("stock_pick.thesis.valuation_metrics", lang, &p) {
            parts.push(text);
        }
    }

    // Growth context
    let mut growth_parts = Vec::new();
    if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy {
        let mut p = serde_json::Map::new();
        p.insert("pct".to_string(), serde_json::json!(rev_yoy * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.revenue_yoy", lang, &p) {
            growth_parts.push(t);
        }
    }
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy {
        let mut p = serde_json::Map::new();
        p.insert("pct".to_string(), serde_json::json!(np_yoy * 100.0));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.net_profit_yoy", lang, &p) {
            growth_parts.push(t);
        }
    }
    if let Some(peg) = item.fundamental_snapshot.peg {
        let mut p = serde_json::Map::new();
        p.insert("peg".to_string(), serde_json::json!(peg));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.peg", lang, &p) {
            growth_parts.push(t);
        }
    }
    if !growth_parts.is_empty() {
        let mut p = serde_json::Map::new();
        p.insert("metrics".to_string(), serde_json::Value::String(growth_parts.join(", ")));
        if let Some(text) = i18n.resolve_with_params("stock_pick.thesis.growth_metrics", lang, &p) {
            parts.push(text);
        }
    }

    // Technical signals
    let mut tech_parts = Vec::new();
    if let Some(rsi) = item.technical_snapshot.rsi {
        let mut p = serde_json::Map::new();
        p.insert("rsi".to_string(), serde_json::json!(rsi));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.rsi", lang, &p) {
            tech_parts.push(t);
        }
    }
    if let Some(macd) = item.technical_snapshot.macd_hist {
        let mut p = serde_json::Map::new();
        p.insert("macd".to_string(), serde_json::json!(macd));
        if let Some(t) = i18n.resolve_with_params("stock_pick.evidence.macd_hist", lang, &p) {
            tech_parts.push(t);
        }
    }
    if let Some(atr) = item.technical_snapshot.atr {
        tech_parts.push(format!("ATR {:.2}", atr));
    }
    if !tech_parts.is_empty() {
        let mut p = serde_json::Map::new();
        p.insert("metrics".to_string(), serde_json::Value::String(tech_parts.join(", ")));
        if let Some(text) = i18n.resolve_with_params("stock_pick.thesis.technical_indicators", lang, &p) {
            parts.push(text);
        }
    }

    // Factor breakdown
    let mut fp = serde_json::Map::new();
    fp.insert("momentum".to_string(), serde_json::json!(item.factor.momentum));
    fp.insert("quality".to_string(), serde_json::json!(item.factor.quality));
    fp.insert("value".to_string(), serde_json::json!(item.factor.value));
    fp.insert("growth".to_string(), serde_json::json!(item.factor.growth));
    fp.insert("profitability".to_string(), serde_json::json!(item.factor.profitability));
    fp.insert("risk".to_string(), serde_json::json!(item.factor.risk));
    if let Some(text) = i18n.resolve_with_params("stock_pick.thesis.factor_scores", lang, &fp) {
        parts.push(text);
    }

    parts.join(" ")
}

/// Generate i18n keys for catalysts with parameters.
pub(crate) fn default_catalyst_keys(item: &EnrichedCandidate) -> Vec<serde_json::Value> {
    let mut keys = Vec::new();
    let mk = |key: &str, params: serde_json::Value| -> serde_json::Value {
        let mut obj = params.as_object().cloned().unwrap_or_default();
        obj.insert("i18n_key".to_string(), serde_json::json!(key));
        serde_json::Value::Object(obj)
    };

    // Price momentum
    if let (Some(first), Some(last)) = (item.candles.first(), item.candles.last())
        && first.close > 0.0
    {
        let ret = ((last.close / first.close) - 1.0) * 100.0;
        if ret > 5.0 {
            keys.push(mk("stock_pick.catalyst.strong_return", serde_json::json!({"pct": ret})));
        }
    }

    // Technical signals
    if let Some(rsi) = item.technical_snapshot.rsi
        && (50.0..70.0).contains(&rsi)
    {
        keys.push(mk("stock_pick.catalyst.rsi_bullish", serde_json::json!({"rsi": rsi})));
    }
    if let Some(macd_hist) = item.technical_snapshot.macd_hist
        && macd_hist > 0.0
    {
        keys.push(mk("stock_pick.catalyst.macd_positive", serde_json::json!({})));
    }

    // Valuation
    if let Some(pe) = item.fundamental_snapshot.pe_like
        && pe < 25.0
    {
        keys.push(mk("stock_pick.catalyst.pe_reasonable", serde_json::json!({"pe": pe})));
    }

    // Earnings
    if let Some(roe) = item.fundamental_snapshot.roe
        && roe > 0.08
    {
        keys.push(mk("stock_pick.catalyst.roe_strong", serde_json::json!({"roe": roe * 100.0})));
    }

    // Growth catalysts
    if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy
        && rev_yoy > 0.15
    {
        keys.push(mk("stock_pick.catalyst.revenue_growth", serde_json::json!({"pct": rev_yoy * 100.0})));
    }
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy
        && np_yoy > 0.2
    {
        keys.push(mk("stock_pick.catalyst.profit_growth", serde_json::json!({"pct": np_yoy * 100.0})));
    }
    if let Some(peg) = item.fundamental_snapshot.peg
        && (0.0..1.0).contains(&peg)
    {
        keys.push(mk("stock_pick.catalyst.peg_undervalued", serde_json::json!({"peg": peg})));
    }

    // Analyst consensus
    if let Some(buy_ratio) = item.fundamental_snapshot.analyst_buy_ratio
        && buy_ratio > 0.6
    {
        keys.push(mk("stock_pick.catalyst.analyst_bullish", serde_json::json!({"pct": buy_ratio * 100.0})));
    }

    // Dividend
    if let Some(dy) = item.fundamental_snapshot.dividend_yield
        && dy > 0.02
    {
        keys.push(mk("stock_pick.catalyst.dividend_yield", serde_json::json!({"pct": dy * 100.0})));
    }

    // Fund flow
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio
        && flow > 0.05
    {
        keys.push(mk("stock_pick.catalyst.fund_flow_positive", serde_json::json!({})));
    }

    // News catalysts
    if item.news_snapshot.catalyst_count > 0 {
        keys.push(mk("stock_pick.catalyst.news_catalyst", serde_json::json!({"count": item.news_snapshot.catalyst_count})));
    }

    // Factor-based catalysts
    if item.factor.total >= 60.0 {
        keys.push(mk("stock_pick.catalyst.factor_strong", serde_json::json!({"score": item.factor.total})));
    }
    if item.factor.quality >= 60.0 {
        keys.push(mk("stock_pick.catalyst.quality_strong", serde_json::json!({})));
    }
    if item.factor.profitability >= 60.0 {
        keys.push(mk("stock_pick.catalyst.profitability_strong", serde_json::json!({})));
    }
    if item.factor.value >= 60.0 {
        keys.push(mk("stock_pick.catalyst.value_attractive", serde_json::json!({})));
    }
    if item.factor.growth >= 60.0 {
        keys.push(mk("stock_pick.catalyst.growth_confirmed", serde_json::json!({})));
    }

    // Ensure at least 2 catalysts
    if keys.is_empty() {
        keys.push(mk("stock_pick.catalyst.default_1", serde_json::json!({})));
        keys.push(mk("stock_pick.catalyst.default_2", serde_json::json!({})));
    } else if keys.len() == 1 {
        keys.push(mk("stock_pick.catalyst.systematic", serde_json::json!({})));
    }
    keys
}

/// Generate i18n keys for risks with parameters.
pub(crate) fn default_risk_keys(item: &EnrichedCandidate) -> Vec<serde_json::Value> {
    let mut keys = Vec::new();
    let mk = |key: &str, params: serde_json::Value| -> serde_json::Value {
        let mut obj = params.as_object().cloned().unwrap_or_default();
        obj.insert("i18n_key".to_string(), serde_json::json!(key));
        serde_json::Value::Object(obj)
    };

    // Valuation risk
    if let Some(pe) = item.fundamental_snapshot.pe_like
        && pe > 50.0
    {
        keys.push(mk("stock_pick.risk.high_pe", serde_json::json!({"pe": pe})));
    }
    if let Some(ps) = item.fundamental_snapshot.ps_like
        && ps > 8.0
    {
        keys.push(mk("stock_pick.risk.high_ps", serde_json::json!({"ps": ps})));
    }

    // Volatility risk
    if let Some(atr) = item.technical_snapshot.atr
        && let Some(price) = item.price
        && price > 0.0
        && atr / price > 0.03
    {
        keys.push(mk("stock_pick.risk.high_volatility", serde_json::json!({})));
    }

    // Overbought risk
    if let Some(rsi) = item.technical_snapshot.rsi
        && rsi > 70.0
    {
        keys.push(mk("stock_pick.risk.rsi_overbought", serde_json::json!({"rsi": rsi})));
    }

    // Recent surge risk
    if item.change_pct.unwrap_or_default() >= 9.5 {
        keys.push(mk("stock_pick.risk.single_day_surge", serde_json::json!({})));
    }

    // Leverage risk
    if let Some(lev) = item.fundamental_snapshot.leverage
        && lev > 1.5
    {
        keys.push(mk("stock_pick.risk.high_leverage", serde_json::json!({"ratio": lev})));
    }

    // Chip risk: low benefit ratio means most holders underwater
    if let Some(benefit) = item.fundamental_snapshot.chip_benefit_ratio
        && benefit < 0.3
    {
        keys.push(mk("stock_pick.risk.chip_underwater", serde_json::json!({"pct": benefit * 100.0})));
    }

    // Growth risk: declining earnings
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy
        && np_yoy < -0.2
    {
        keys.push(mk("stock_pick.risk.profit_decline", serde_json::json!({"pct": np_yoy * 100.0})));
    }

    // PB valuation risk
    if let Some(pb) = item.fundamental_snapshot.pb
        && pb > 8.0
    {
        keys.push(mk("stock_pick.risk.high_pb", serde_json::json!({"pb": pb})));
    }

    // Chip concentration risk: high concentration = more volatile on large trades
    if let Some(conc) = item.fundamental_snapshot.chip_concentration_90
        && conc > 0.7
    {
        keys.push(mk("stock_pick.risk.chip_concentrated", serde_json::json!({})));
    }

    // Fund flow risk: heavy outflows
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio
        && flow < -0.1
    {
        keys.push(mk("stock_pick.risk.fund_flow_negative", serde_json::json!({})));
    }

    // Factor-based risks
    if item.factor.risk < 50.0 {
        keys.push(mk("stock_pick.risk.risk_low_score", serde_json::json!({"score": item.factor.risk})));
    }
    if item.factor.momentum < 40.0 {
        keys.push(mk("stock_pick.risk.momentum_weak", serde_json::json!({"score": item.factor.momentum})));
    }
    if item.factor.growth < 40.0 {
        keys.push(mk("stock_pick.risk.growth_weak", serde_json::json!({"score": item.factor.growth})));
    }

    // Data quality risks
    if item.fundamentals.is_none() {
        keys.push(mk("stock_pick.risk.limited_data", serde_json::json!({})));
    }
    if item.candles.len() < 30 {
        keys.push(mk("stock_pick.risk.short_history", serde_json::json!({})));
    }

    // Market-level risks
    keys.push(mk("stock_pick.risk.macro_uncertainty", serde_json::json!({})));

    // Ensure at least 2 risks
    if keys.len() < 2 {
        keys.push(mk("stock_pick.risk.monitor_dynamics", serde_json::json!({})));
    }
    keys
}

/// Generate i18n key for thesis with parameters.
pub(crate) fn default_thesis_key(item: &EnrichedCandidate, i18n: &I18n, lang: &str) -> serde_json::Value {
    let mk = |key: &str, params: serde_json::Value| -> serde_json::Value {
        let mut obj = params.as_object().cloned().unwrap_or_default();
        obj.insert("i18n_key".to_string(), serde_json::json!(key));
        serde_json::Value::Object(obj)
    };

    let (first, last) = match (item.candles.first(), item.candles.last()) {
        (Some(f), Some(l)) if f.close > 0.0 => (f, l),
        _ => {
            return mk("stock_pick.thesis.market_context", serde_json::json!({
                "name": item.name, "symbol": item.symbol, "market": item.market, "industry": item.industry
            }));
        }
    };
    let ret = ((last.close / first.close) - 1.0) * 100.0;
    let direction = if ret > 5.0 {
        i18n.resolve("stock_pick.thesis.direction.strong_bullish", lang).unwrap_or_else(|| "bullish".to_string())
    } else if ret > 0.0 {
        i18n.resolve("stock_pick.thesis.direction.moderate", lang).unwrap_or_else(|| "slightly bullish".to_string())
    } else {
        i18n.resolve("stock_pick.thesis.direction.bearish", lang).unwrap_or_else(|| "bearish".to_string())
    };

    let pe = item.fundamental_snapshot.pe_like.unwrap_or(0.0);
    let ps = item.fundamental_snapshot.ps_like.unwrap_or(0.0);
    let roe = item.fundamental_snapshot.roe.unwrap_or(0.0) * 100.0;
    let pb = item.fundamental_snapshot.pb.unwrap_or(0.0);
    let rev_yoy = item.fundamental_snapshot.revenue_yoy.unwrap_or(0.0) * 100.0;
    let np_yoy = item.fundamental_snapshot.net_profit_yoy.unwrap_or(0.0) * 100.0;
    let peg = item.fundamental_snapshot.peg.unwrap_or(0.0);
    let rsi = item.technical_snapshot.rsi.unwrap_or(50.0);
    let rsi_label = if rsi > 70.0 { "overbought" } else if rsi > 55.0 { "bullish" } else if rsi > 45.0 { "neutral" } else { "oversold" };
    let macd = item.technical_snapshot.macd_hist.unwrap_or(0.0);
    let atr = item.technical_snapshot.atr.unwrap_or(0.0);

    mk("stock_pick.thesis.price_action", serde_json::json!({
        "name": item.name, "symbol": item.symbol,
        "start_price": first.close, "end_price": last.close, "ret": ret, "direction": direction,
        "pe": pe, "ps": ps, "roe": roe, "pb": pb,
        "rev_yoy": rev_yoy, "np_yoy": np_yoy, "peg": peg,
        "rsi": rsi, "rsi_label": rsi_label, "macd": macd, "atr": atr,
        "momentum": item.factor.momentum, "quality": item.factor.quality,
        "value": item.factor.value, "growth": item.factor.growth,
        "profitability": item.factor.profitability, "risk": item.factor.risk,
        "market": item.market, "industry": item.industry
    }))
}

/// Generate i18n keys for evidence points.
pub(crate) fn default_evidence_keys(item: &EnrichedCandidate) -> Vec<serde_json::Value> {
    let mut keys = Vec::new();
    let mk = |key: &str, params: serde_json::Value| -> serde_json::Value {
        let mut obj = params.as_object().cloned().unwrap_or_default();
        obj.insert("i18n_key".to_string(), serde_json::json!(key));
        serde_json::Value::Object(obj)
    };

    // Price range
    if let (Some(first), Some(last)) = (item.candles.first(), item.candles.last())
        && first.close > 0.0
    {
        let ret = ((last.close / first.close) - 1.0) * 100.0;
        keys.push(mk("stock_pick.evidence.price_range", serde_json::json!({
            "start_date": first.trade_date, "end_date": last.trade_date,
            "start_price": first.close, "end_price": last.close, "ret": ret
        })));
        keys.push(mk("stock_pick.evidence.latest", serde_json::json!({
            "change_pct": last.change_pct, "volume": last.volume
        })));
    }

    // Market cap
    if let Some(mc) = item.market_cap {
        if mc >= 1_000_000_000_000.0 {
            keys.push(mk("stock_pick.evidence.market_cap_t", serde_json::json!({"cap": mc / 1_000_000_000_000.0})));
        } else if mc >= 100_000_000.0 {
            keys.push(mk("stock_pick.evidence.market_cap_b", serde_json::json!({"cap": mc / 100_000_000.0})));
        }
    }

    // Fundamentals
    if let Some(pe) = item.fundamental_snapshot.pe_like {
        keys.push(mk("stock_pick.evidence.pe", serde_json::json!({"pe": pe})));
    }
    if let Some(roe) = item.fundamental_snapshot.roe {
        keys.push(mk("stock_pick.evidence.roe", serde_json::json!({"roe": roe * 100.0})));
    }
    if let Some(pe_ttm) = item.fundamental_snapshot.pe_ttm {
        keys.push(mk("stock_pick.evidence.pe_ttm", serde_json::json!({"pe_ttm": pe_ttm})));
    }
    if let Some(pb) = item.fundamental_snapshot.pb {
        keys.push(mk("stock_pick.evidence.pb", serde_json::json!({"pb": pb})));
    }
    if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy {
        keys.push(mk("stock_pick.evidence.revenue_yoy", serde_json::json!({"pct": rev_yoy * 100.0})));
    }
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy {
        keys.push(mk("stock_pick.evidence.net_profit_yoy", serde_json::json!({"pct": np_yoy * 100.0})));
    }
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio {
        keys.push(mk("stock_pick.evidence.fund_flow", serde_json::json!({"pct": flow * 100.0})));
    }
    if let Some(count) = item.fundamental_snapshot.analyst_report_count {
        keys.push(mk("stock_pick.evidence.analyst_reports", serde_json::json!({"count": count})));
    }
    if let Some(gm) = item.fundamental_snapshot.gross_margin {
        keys.push(mk("stock_pick.evidence.gross_margin", serde_json::json!({"pct": gm * 100.0})));
    }
    if let Some(peg) = item.fundamental_snapshot.peg {
        keys.push(mk("stock_pick.evidence.peg", serde_json::json!({"peg": peg})));
    }

    keys
}

/// Generate i18n key for objective headline.
pub(crate) fn default_headline_key(final_score: i32, ready: bool, gaps: &[String], i18n: &I18n, lang: &str) -> serde_json::Value {
    let mk = |key: &str, params: serde_json::Value| -> serde_json::Value {
        let mut obj = params.as_object().cloned().unwrap_or_default();
        obj.insert("i18n_key".to_string(), serde_json::json!(key));
        serde_json::Value::Object(obj)
    };

    if ready && final_score >= 85 {
        mk("stock_pick.headline.high_quality", serde_json::json!({}))
    } else if ready {
        mk("stock_pick.headline.usable", serde_json::json!({}))
    } else if gaps.is_empty() {
        mk("stock_pick.headline.mixed", serde_json::json!({}))
    } else {
        let translated_gaps: Vec<String> = gaps.iter().map(|g| {
            i18n.resolve(&format!("stock_pick.gap.{}", g), lang)
                .unwrap_or_else(|| g.clone())
        }).collect();
        mk("stock_pick.headline.not_ready", serde_json::json!({"gaps": translated_gaps.join(", ")}))
    }
}

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

fn stock_pick_objective_grade(score: i32) -> &'static str {
    match score {
        85..=100 => "A",
        75..=84 => "B",
        60..=74 => "C",
        _ => "D",
    }
}

fn stock_pick_objective_gaps(pick: &StockPickItem, item: &EnrichedCandidate, industry_avg: &IndustryAverages) -> Vec<String> {
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

fn stock_pick_objective_headline(score: i32, ready: bool, gaps: &[String], i18n: &I18n, lang: &str) -> String {
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
        p.insert("roe".to_string(), serde_json::json!(roe * 100.0));
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


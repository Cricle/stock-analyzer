use std::collections::{HashMap, HashSet};

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

pub(crate) fn compute_industry_averages(
    all_candidates: &[EnrichedCandidate],
) -> HashMap<String, IndustryAverages> {
    let mut pe_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut ps_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut roe_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut pe_ttm_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut pb_sums: HashMap<String, Vec<f64>> = HashMap::new();
    let mut gm_sums: HashMap<String, Vec<f64>> = HashMap::new();

    for candidate in all_candidates {
        let industry = &candidate.industry;
        if industry == "Unknown" {
            continue;
        }
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
) -> Option<String> {
    let v = value?;
    if !v.is_finite() || v <= 0.0 {
        return None;
    }
    let premium = v / avg;
    let direction = if premium >= 1.0 { "premium" } else { "discount" };
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
        if let Some(line) = format_valuation_line(
            "PE",
            candidate.fundamental_snapshot.pe_like,
            avg.pe_avg,
        ) {
            parts.push(line);
        }
        if let Some(line) = format_valuation_line(
            "PS",
            candidate.fundamental_snapshot.ps_like,
            avg.ps_avg,
        ) {
            parts.push(line);
        }
        if let Some(line) = format_valuation_line(
            "PE_TTM",
            candidate.fundamental_snapshot.pe_ttm.filter(|v| *v > 0.0),
            avg.pe_ttm_avg,
        ) {
            parts.push(line);
        }
        if let Some(line) = format_valuation_line(
            "PB",
            candidate.fundamental_snapshot.pb.filter(|v| *v > 0.0),
            avg.pb_avg,
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
    let mut parts = Vec::new();

    // Price action with context
    if let (Some(first), Some(last)) = (item.candles.first(), item.candles.last()) {
        if first.close > 0.0 {
            let ret = ((last.close / first.close) - 1.0) * 100.0;
            let direction = if ret >= 0.0 { "gained" } else { "declined" };
            parts.push(format!(
                "{} {} {:.1}% over the analysis period (from {:.2} to {:.2}), demonstrating {} price action.",
                item.name, direction, ret.abs(), first.close, last.close,
                if ret >= 5.0 { "strong bullish" } else if ret >= 0.0 { "moderate" } else { "bearish" }
            ));
        }
    } else {
        parts.push(format!(
            "{} shows a composite factor score of {:.1}, with momentum at {:.1} and quality at {:.1}.",
            item.name, item.factor.total, item.factor.momentum, item.factor.quality
        ));
    }

    // Valuation context
    let mut val_parts = Vec::new();
    if let Some(pe) = item.fundamental_snapshot.pe_like {
        val_parts.push(format!("PE {:.1}x", pe));
    }
    if let Some(ps) = item.fundamental_snapshot.ps_like {
        val_parts.push(format!("PS {:.1}x", ps));
    }
    if let Some(roe) = item.fundamental_snapshot.roe {
        val_parts.push(format!("ROE {:.1}%", roe * 100.0));
    }
    if let Some(pb) = item.fundamental_snapshot.pb {
        val_parts.push(format!("PB {:.1}x", pb));
    }
    if !val_parts.is_empty() {
        parts.push(format!("Valuation metrics: {}.", val_parts.join(", ")));
    }

    // Growth context
    let mut growth_parts = Vec::new();
    if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy {
        growth_parts.push(format!("Revenue YoY {:.1}%", rev_yoy * 100.0));
    }
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy {
        growth_parts.push(format!("Net Profit YoY {:.1}%", np_yoy * 100.0));
    }
    if let Some(peg) = item.fundamental_snapshot.peg {
        growth_parts.push(format!("PEG {:.2}x", peg));
    }
    if !growth_parts.is_empty() {
        parts.push(format!("Growth metrics: {}.", growth_parts.join(", ")));
    }

    // Technical signals
    let mut tech_parts = Vec::new();
    if let Some(rsi) = item.technical_snapshot.rsi {
        let rsi_label = if rsi > 70.0 { "overbought" } else if rsi > 55.0 { "bullish" } else if rsi > 45.0 { "neutral" } else { "oversold" };
        tech_parts.push(format!("RSI at {:.1} ({})", rsi, rsi_label));
    }
    if let Some(macd) = item.technical_snapshot.macd_hist {
        tech_parts.push(format!("MACD histogram {:.2}", macd));
    }
    if let Some(atr) = item.technical_snapshot.atr {
        tech_parts.push(format!("ATR {:.2}", atr));
    }
    if !tech_parts.is_empty() {
        parts.push(format!("Technical indicators: {}.", tech_parts.join(", ")));
    }

    // Factor breakdown
    parts.push(format!(
        "Factor scores: momentum {:.0}, quality {:.0}, value {:.0}, growth {:.0}, profitability {:.0}, risk {:.0}.",
        item.factor.momentum, item.factor.quality, item.factor.value,
        item.factor.growth, item.factor.profitability, item.factor.risk
    ));

    parts.join(" ")
}

pub(crate) fn default_catalysts(item: &EnrichedCandidate) -> Vec<String> {
    let mut catalysts = Vec::new();

    // Price momentum
    if let (Some(first), Some(last)) = (item.candles.first(), item.candles.last())
        && first.close > 0.0
    {
        let ret = ((last.close / first.close) - 1.0) * 100.0;
        if ret > 5.0 {
            catalysts.push(format!("Strong period return of {:.1}%", ret));
        }
    }

    // Technical signals
    if let Some(rsi) = item.technical_snapshot.rsi
        && (50.0..70.0).contains(&rsi)
    {
        catalysts.push(format!("RSI at {:.1} indicates bullish momentum without overbought risk", rsi));
    }
    if let Some(macd_hist) = item.technical_snapshot.macd_hist
        && macd_hist > 0.0
    {
        catalysts.push("MACD histogram positive, confirming uptrend".to_string());
    }

    // Valuation
    if let Some(pe) = item.fundamental_snapshot.pe_like
        && pe < 25.0
    {
        catalysts.push(format!("PE ratio {:.1}x is reasonable for growth potential", pe));
    }

    // Earnings
    if let Some(roe) = item.fundamental_snapshot.roe
        && roe > 0.08
    {
        catalysts.push(format!("ROE of {:.1}% demonstrates efficient capital utilization", roe * 100.0));
    }

    // Growth catalysts
    if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy
        && rev_yoy > 0.15
    {
        catalysts.push(format!("Revenue YoY growth of {:.1}% shows strong demand", rev_yoy * 100.0));
    }
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy
        && np_yoy > 0.2
    {
        catalysts.push(format!("Net profit YoY growth of {:.1}% demonstrates earnings acceleration", np_yoy * 100.0));
    }
    if let Some(peg) = item.fundamental_snapshot.peg
        && (0.0..1.0).contains(&peg)
    {
        catalysts.push(format!("PEG ratio {:.2}x indicates undervalued growth", peg));
    }

    // Analyst consensus
    if let Some(buy_ratio) = item.fundamental_snapshot.analyst_buy_ratio
        && buy_ratio > 0.6
    {
        catalysts.push(format!("Strong analyst consensus with {:.0}% buy/overweight ratings", buy_ratio * 100.0));
    }

    // Dividend
    if let Some(dy) = item.fundamental_snapshot.dividend_yield
        && dy > 0.02
    {
        catalysts.push(format!("Dividend yield of {:.1}% provides income support", dy * 100.0));
    }

    // Fund flow
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio
        && flow > 0.05
    {
        catalysts.push("Positive fund flow indicates institutional accumulation".to_string());
    }

    // News catalysts
    if item.news_snapshot.catalyst_count > 0 {
        catalysts.push(format!("{} positive news catalysts identified", item.news_snapshot.catalyst_count));
    }

    // Factor-based catalysts
    if item.factor.total >= 60.0 {
        catalysts.push(format!("Strong composite factor score of {:.1} indicates favorable risk-reward", item.factor.total));
    }
    if item.factor.quality >= 60.0 {
        catalysts.push("Quality metrics support sustained fundamental strength".to_string());
    }
    if item.factor.profitability >= 60.0 {
        catalysts.push("Profitability indicators suggest earnings resilience".to_string());
    }
    if item.factor.value >= 60.0 {
        catalysts.push("Value metrics indicate attractive entry point relative to peers".to_string());
    }
    if item.factor.growth >= 60.0 {
        catalysts.push("Growth metrics confirm expansion trajectory".to_string());
    }

    // Ensure at least 2 catalysts
    if catalysts.is_empty() {
        catalysts.push("Composite factor score leads the candidate pool".to_string());
        catalysts.push("Technical and fundamental alignment supports the selection thesis".to_string());
    } else if catalysts.len() == 1 {
        catalysts.push("Systematic factor analysis confirms selection rationale".to_string());
    }
    catalysts
}

pub(crate) fn default_risks(item: &EnrichedCandidate) -> Vec<String> {
    let mut risks = Vec::new();

    // Valuation risk
    if let Some(pe) = item.fundamental_snapshot.pe_like
        && pe > 50.0
    {
        risks.push(format!("High PE of {:.1}x implies elevated valuation risk", pe));
    }
    if let Some(ps) = item.fundamental_snapshot.ps_like
        && ps > 8.0
    {
        risks.push(format!("PS ratio {:.1}x suggests premium pricing", ps));
    }

    // Volatility risk
    if let Some(atr) = item.technical_snapshot.atr
        && let Some(price) = item.price
        && price > 0.0
        && atr / price > 0.03
    {
        risks.push("Elevated ATR/price ratio indicates above-average volatility".to_string());
    }

    // Overbought risk
    if let Some(rsi) = item.technical_snapshot.rsi
        && rsi > 70.0
    {
        risks.push(format!("RSI at {:.1} suggests overbought conditions", rsi));
    }

    // Recent surge risk
    if item.change_pct.unwrap_or_default() >= 9.5 {
        risks.push("Large single-day gain increases near-term pullback risk".to_string());
    }

    // Leverage risk
    if let Some(lev) = item.fundamental_snapshot.leverage
        && lev > 1.5
    {
        risks.push(format!("Debt/equity ratio of {:.2} implies higher financial leverage", lev));
    }

    // Chip risk: low benefit ratio means most holders underwater
    if let Some(benefit) = item.fundamental_snapshot.chip_benefit_ratio
        && benefit < 0.3
    {
        risks.push(format!("Only {:.0}% of holders in profit, elevated selloff pressure", benefit * 100.0));
    }

    // Growth risk: declining earnings
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy
        && np_yoy < -0.2
    {
        risks.push(format!("Net profit declined {:.1}% YoY, earnings deterioration risk", np_yoy * 100.0));
    }

    // PB valuation risk
    if let Some(pb) = item.fundamental_snapshot.pb
        && pb > 8.0
    {
        risks.push(format!("PB ratio {:.1}x indicates premium asset valuation", pb));
    }

    // Chip concentration risk: high concentration = more volatile on large trades
    if let Some(conc) = item.fundamental_snapshot.chip_concentration_90
        && conc > 0.7
    {
        risks.push("High chip concentration increases volatility on large trades".to_string());
    }

    // Fund flow risk: heavy outflows
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio
        && flow < -0.1
    {
        risks.push("Negative fund flow indicates institutional distribution".to_string());
    }

    // Factor-based risks
    if item.factor.risk < 50.0 {
        risks.push("Risk factor score below average warrants position sizing caution".to_string());
    }
    if item.factor.momentum < 40.0 {
        risks.push("Weak momentum signals suggest potential for continued underperformance".to_string());
    }
    if item.factor.growth < 40.0 {
        risks.push("Growth metrics below average raises sustainability concerns".to_string());
    }

    // Data quality risks
    if item.fundamentals.is_none() {
        risks.push("Limited fundamental data availability constrains valuation confidence".to_string());
    }
    if item.candles.len() < 30 {
        risks.push("Short price history limits statistical reliability of technical signals".to_string());
    }

    // Market-level risks
    risks.push("Macroeconomic conditions and sector rotation dynamics remain key uncertainties".to_string());

    // Ensure at least 2 risks
    if risks.len() < 2 {
        risks.push("Monitor price-volume dynamics and upcoming earnings announcements".to_string());
    }
    risks
}

pub(crate) fn evaluate_stock_pick_objective_assessment(
    pick: &StockPickItem,
    item: &EnrichedCandidate,
    metrics: &AdvancedMetrics,
    industry_avg: &IndustryAverages,
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
    // Enrichment gaps
    if item.enrichment.pe_ttm.is_none() && item.enrichment.pb.is_none() {
        gaps.push("missing_valuation_enrichment".to_string());
    }
    if item.enrichment.revenue_yoy.is_none() && item.enrichment.net_profit_yoy.is_none() {
        gaps.push("missing_earnings_growth".to_string());
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

    // Price data
    if let (Some(first), Some(last)) = (item.candles.first(), item.candles.last()) {
        if first.close > 0.0 {
            let ret = ((last.close / first.close) - 1.0) * 100.0;
            evidence.push(format!(
                "{} to {} price {:.2} -> {:.2} ({:+.1}%)",
                first.trade_date, last.trade_date, first.close, last.close, ret
            ));
        }
        evidence.push(format!(
            "Latest: change {:.2}%, volume {}",
            last.change_pct, last.volume
        ));
    }

    // Market cap
    if let Some(mc) = item.market_cap {
        if mc >= 1_000_000_000_000.0 {
            evidence.push(format!("Market cap {:.0}T yuan", mc / 1_000_000_000_000.0));
        } else if mc >= 100_000_000.0 {
            evidence.push(format!("Market cap {:.0}B yuan", mc / 100_000_000.0));
        }
    }

    // Fundamentals
    if let Some(pe) = item.fundamental_snapshot.pe_like {
        evidence.push(format!("PE {:.1}x", pe));
    }
    if let Some(roe) = item.fundamental_snapshot.roe {
        evidence.push(format!("ROE {:.1}%", roe * 100.0));
    }

    // Enrichment data
    if let Some(pe_ttm) = item.fundamental_snapshot.pe_ttm {
        evidence.push(format!("PE TTM {:.1}x", pe_ttm));
    }
    if let Some(pb) = item.fundamental_snapshot.pb {
        evidence.push(format!("PB {:.1}x", pb));
    }
    if let Some(rev_yoy) = item.fundamental_snapshot.revenue_yoy {
        evidence.push(format!("Revenue YoY {:.1}%", rev_yoy * 100.0));
    }
    if let Some(np_yoy) = item.fundamental_snapshot.net_profit_yoy {
        evidence.push(format!("Net Profit YoY {:.1}%", np_yoy * 100.0));
    }
    if let Some(flow) = item.fundamental_snapshot.fund_flow_net_ratio {
        evidence.push(format!("Fund flow net ratio {:.2}%", flow * 100.0));
    }
    if let Some(count) = item.fundamental_snapshot.analyst_report_count {
        evidence.push(format!("Analyst reports: {}", count));
    }
    if let Some(gm) = item.fundamental_snapshot.gross_margin {
        evidence.push(format!("Gross margin {:.1}%", gm * 100.0));
    }
    if let Some(peg) = item.fundamental_snapshot.peg {
        evidence.push(format!("PEG {:.2}x", peg));
    }
    if let Some(dy) = item.fundamental_snapshot.dividend_yield {
        evidence.push(format!("Dividend yield {:.2}%", dy * 100.0));
    }
    if let Some(chip) = item.fundamental_snapshot.chip_benefit_ratio {
        evidence.push(format!("Chip benefit ratio {:.0}%", chip * 100.0));
    }

    // Technical
    if let Some(rsi) = item.technical_snapshot.rsi {
        evidence.push(format!("RSI {:.1}", rsi));
    }
    if let Some(macd_hist) = item.technical_snapshot.macd_hist {
        evidence.push(format!("MACD hist {:.2}", macd_hist));
    }

    // Factor scores
    evidence.push(format!(
        "Factor: total {:.1}, momentum {:.1}, quality {:.1}, value {:.1}, growth {:.1}",
        item.factor.total, item.factor.momentum, item.factor.quality, item.factor.value,
        item.factor.growth
    ));

    // Additional factor details
    evidence.push(format!(
        "Factor: profitability {:.1}, risk {:.1}, event {:.1}",
        item.factor.profitability, item.factor.risk, item.factor.event
    ));

    // News
    if !item.news.is_empty() {
        evidence.push(format!("{} recent news items from {} unique sources",
            item.news.len(), item.news_snapshot.unique_source_count));
    }

    // Candle stats
    if item.candles.len() >= 20 {
        let up_days = item.candles.windows(2)
            .filter(|w| w[1].close >= w[0].close)
            .count();
        let total = item.candles.windows(2).count().max(1);
        evidence.push(format!("{} candles with {:.0}% up-day ratio", item.candles.len(), up_days as f64 / total as f64 * 100.0));
    }

    // Industry
    if item.industry != "Unknown" {
        evidence.push(format!("Industry: {}", item.industry));
    }

    evidence
}


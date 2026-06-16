use crate::i18n::I18n;
use crate::engine::stock_pick::EnrichedCandidate;

use super::compute_industry_averages;
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
        let roe_pct = roe * 100.0;
        let mut p = serde_json::Map::new();
        // Annotate extreme ROE values that likely indicate negative equity
        if roe_pct.abs() > 100.0 {
            let annotation = i18n.resolve("stock_pick.evidence.negative_equity", lang).unwrap_or_default();
            p.insert("roe".to_string(), serde_json::json!(format!("{:.1}% ({annotation})", roe_pct)));
        } else {
            p.insert("roe".to_string(), serde_json::json!(roe_pct));
        }
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
            let days = item.candles.len();
            keys.push(mk("stock_pick.catalyst.strong_return", serde_json::json!({"pct": ret, "days": days})));
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

    // Growth catalysts — skip if PEG is negative (low base / unsustainable)
    let peg_is_negative = item.fundamental_snapshot.peg.is_some_and(|v| v < 0.0);
    if !peg_is_negative {
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
    let roe_raw = item.fundamental_snapshot.roe.unwrap_or(0.0) * 100.0;
    // Annotate extreme ROE values that likely indicate negative equity
    let roe = if roe_raw.abs() > 100.0 {
        let annotation = i18n.resolve("stock_pick.evidence.negative_equity", lang).unwrap_or_default();
        format!("{:.1}% ({annotation})", roe_raw)
    } else {
        format!("{:.1}", roe_raw)
    };
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

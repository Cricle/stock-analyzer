use crate::{StockPickItem, StockPickObjectiveBucket, StockPickObjectiveOverview};

use crate::pick::EnrichedCandidate;

use super::constraints::{build_valuation_vs_industry_block, stock_pick_objective_grade};

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
         For override_actions, action must be one of: \"remove\", \"raise\", \"lower\".\n\
         For any difference, provide override_actions explaining WHY the evidence supports your alternative.\n\
         Disagreement is expected and healthy when evidence warrants it.\n\n\
         ## CRITICAL: Actionable Recommendations\n\
         For EACH pick, you MUST provide actionable trading guidance:\n\
         - entry_price: Specific price or price range for entry (e.g., \"150.00\" or \"150-155\")\n\
         - stop_loss: Specific stop-loss price (e.g., \"145.00\")\n\
         - target_price: Realistic price target with justification (e.g., \"175.00 based on resistance\")\n\
         - holding_period: Expected holding period (e.g., \"2-4 weeks\", \"1-3 months\")\n\
         - exit_triggers: Specific conditions that would trigger exit (e.g., [\"break below 145\", \"earnings miss\"])\n\n\
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
               \"data_gaps\": [\"missing_history\", \"missing_fundamentals\"],\n\
               \"entry_price\": \"specific price or range\",\n\
               \"stop_loss\": \"specific stop price\",\n\
               \"target_price\": \"specific target price\",\n\
               \"holding_period\": \"expected duration\",\n\
               \"exit_triggers\": [\"condition1\", \"condition2\"]\n\
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
        "pick.default_thesis|name={}|total={:.1}|momentum={:.1}|quality={:.1}|value={:.1}|profitability={:.1}|risk={:.1}|event={:.1}",
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
        catalysts.push("pick.catalyst.strong_momentum".to_string());
    }
    if item.factor.event >= 60.0 {
        catalysts.push("pick.catalyst.clear_catalyst".to_string());
    }
    if item.factor.quality >= 60.0 {
        catalysts.push("pick.catalyst.acceptable_quality".to_string());
    }
    if catalysts.is_empty() {
        catalysts.push("pick.catalyst.leading_composite_score".to_string());
    }
    catalysts
}

pub(crate) fn default_risks(item: &EnrichedCandidate) -> Vec<String> {
    let mut risks = Vec::new();
    if item.change_pct.unwrap_or_default() >= 9.5 {
        risks.push("pick.risk.large_short_term_gain".to_string());
    }
    if item.factor.value < 45.0 {
        risks.push("pick.risk.average_valuation".to_string());
    }
    if item.factor.risk < 50.0 {
        risks.push("pick.risk.elevated_volatility".to_string());
    }
    if risks.is_empty() {
        risks.push("pick.risk.continue_tracking".to_string());
    }
    risks
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

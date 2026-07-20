use crate::{StockPickItem, StockPickObjectiveBucket, StockPickObjectiveOverview};

use crate::guide::I18nText;
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
            let news_headlines = item
                .news_snapshot
                .headline_titles
                .iter()
                .take(5)
                .map(|h| format!("  - {}", h))
                .collect::<Vec<_>>()
                .join("\n");
            let evidence_lines = item
                .evidence_records
                .iter()
                .take(5)
                .map(|e| format!("  - [{}] {}", e.source, e.title))
                .collect::<Vec<_>>()
                .join("\n");
            let analyst_block = "unavailable".to_string();
            format!(
                "Candidate {}\nSymbol: {}\nName: {}\nIndustry: {}\nFactor Total: {:.2}\nMarket Snapshot: price={:?}, change_pct={:?}, period_return_pct={:?}, volume_ratio={:?}\nTechnical Snapshot: rsi={:?}, macd_hist={:?}, ema10={:?}, sma50={:?}, sma200={:?}, atr={:?}, adx={:?}\nFundamental Snapshot: market_cap={:?}, pe_like={:?} (annualized), ps_like={:?}, roe={:?} (latest quarter), leverage={:?}\nAnalyst Consensus: {}\nNews Snapshot: deep_items={}, unique_sources={}, latest_published_at={}\nHistory Snapshot: samples={}, hit_rate={:?}, avg_alpha={:?}\nRisk Flags: {}\nData Gaps: {}\nRecent News:\n{}\nEvidence:\n{}\n",
                index + 1,
                item.symbol,
                item.name,
                item.fundamental_snapshot.industry,
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
                analyst_block,
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
                },
                if news_headlines.is_empty() {
                    "  - unavailable".to_string()
                } else {
                    news_headlines
                },
                if evidence_lines.is_empty() {
                    "  - unavailable".to_string()
                } else {
                    evidence_lines
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
         ## Industry-Specific Valuation Rules\n\
         - Banks & Financial Institutions: PB (price-to-book) is the PRIMARY valuation anchor, NOT PE. \
           PB < 1.0 means trading below book value — typical for banks. Compare PB to historical range and peers. \
           ROE matters more than PE for banks: sustainable ROE above cost of equity supports premium PB.\n\
         - Asset-Heavy Industries (real estate, utilities): Use PB alongside PE. Asset values drive valuation.\n\
         - Growth / Tech: PE and PS are primary. PB is less relevant for asset-light models.\n\
         - Dividends: When a stock announces dividends with yield > 2%, treat as a MATERIAL catalyst (cash return to shareholders), NOT routine disclosure. \
           Large buybacks (>1% of shares) are similarly material.\n\
         - Institutional Targets: When analyst consensus target prices are available, reference them as a benchmark. \
           Your target should not wildly diverge from institutional consensus without strong justification.\n\
         - Technical Indicator Honesty: Describe indicators ACCURATELY, not optimistically. \
           When MACD histogram absolute value < 0.1, say \"MACD near zero, momentum weak/converging\" — NOT \"positive and sustained\". \
           When RSI is 45-55, say \"RSI near 50, balanced\" — NOT \"bullish momentum\". \
           Match description intensity to actual indicator magnitude.\n\n\
         ## Phase 2: Your Independent Picks\n\
         Select your top picks from the candidates above based purely on the evidence.\n\
         For each pick, write a SUBSTANTIVE thesis (at least 2-3 sentences) grounded in specific data points:\n\
         - Reference specific technical indicators (RSI, MACD, EMA crossovers)\n\
         - Cite recent news headlines or evidence that support the thesis\n\
         - Mention fundamental metrics appropriate to the industry (PE/PS for growth, PB for banks/assets, ROE for profitability)\n\
         - Explain WHY this stock is worth picking, not just WHAT it is\n\
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
         For EACH pick, you MUST provide actionable trading guidance with TECHNICAL DERIVATION:\n\
         - entry_price: Specific price or price range for entry (e.g., \"150.00\" or \"150-155\"). MUST cite technical basis: support level, EMA, VWAP, or consolidation zone.\n\
         - stop_loss: Specific stop-loss price (e.g., \"145.00\"). MUST explain why this level (below support, ATR-based, or % risk).\n\
         - target_price: Realistic price target with justification. MUST cite resistance level, measured move, OR institutional consensus target. \
           When institutional targets are available in evidence, your target MUST NOT be below the most conservative institutional target unless you have strong technical evidence for a lower level. \
           State the institutional range if available.\n\
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
               \"entry_rationale\": \"technical basis for entry (support, EMA, VWAP, etc.)\",\n\
               \"stop_loss\": \"specific stop price\",\n\
               \"stop_rationale\": \"basis for stop level\",\n\
               \"target_price\": \"specific target price\",\n\
               \"target_rationale\": \"basis for target (resistance, measured move, etc.)\",\n\
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

pub(crate) fn default_thesis(item: &EnrichedCandidate) -> I18nText {
    let mut thesis = I18nText::new("pick.default_thesis")
        .with_param("name", item.name.clone())
        .with_param("total", item.factor.total)
        .with_param("momentum", item.factor.momentum)
        .with_param("quality", item.factor.quality)
        .with_param("value", item.factor.value)
        .with_param("profitability", item.factor.profitability)
        .with_param("risk", item.factor.risk)
        .with_param("event", item.factor.event);

    // Add industry context if available
    if let Some(ref fundamentals) = item.fundamentals
        && let Some(ref industry) = fundamentals.industry
    {
        thesis = thesis.with_param("industry", industry.clone());
    }

    // Add technical signals
    if let Some(rsi) = item.technical_snapshot.rsi {
        thesis = thesis.with_param("rsi", rsi);
    }
    if let Some(macd_hist) = item.technical_snapshot.macd_hist {
        thesis = thesis.with_param("macd_hist", macd_hist);
    }

    // Add price context
    if let Some(price) = item.price {
        thesis = thesis.with_param("current_price", price);
    }
    if let Some(change_pct) = item.change_pct {
        thesis = thesis.with_param("change_pct", change_pct);
    }

    // Add valuation context
    if let Some(pe) = item.fundamental_snapshot.pe_like {
        thesis = thesis.with_param("pe_like", pe);
    }
    if let Some(roe) = item.fundamental_snapshot.roe {
        thesis = thesis.with_param("roe", roe);
    }

    // Add news context
    if !item.news.is_empty() {
        let news_count = item.news.len();
        thesis = thesis.with_param("news_count", news_count as i64);
        if let Some(latest) = item.news.first() {
            thesis = thesis.with_param("latest_news_title", latest.title.clone());
        }
    }

    thesis
}

pub(crate) fn default_catalysts(item: &EnrichedCandidate) -> Vec<I18nText> {
    let mut catalysts = Vec::new();

    // Technical catalysts
    if item.factor.momentum >= 70.0 {
        catalysts.push(I18nText::new("pick.catalyst.strong_momentum"));
    }
    if let Some(rsi) = item.technical_snapshot.rsi {
        if rsi < 30.0 {
            catalysts.push(I18nText::new("pick.catalyst.oversold_rsi"));
        } else if rsi > 50.0 && rsi < 70.0 {
            catalysts.push(I18nText::new("pick.catalyst.bullish_rsi"));
        }
    }
    if let Some(macd_hist) = item.technical_snapshot.macd_hist
        && macd_hist > 0.0
    {
        catalysts.push(I18nText::new("pick.catalyst.bullish_macd"));
    }

    // Fundamental catalysts
    if item.factor.event >= 60.0 {
        catalysts.push(I18nText::new("pick.catalyst.clear_catalyst"));
    }
    if item.factor.quality >= 60.0 {
        catalysts.push(I18nText::new("pick.catalyst.acceptable_quality"));
    }
    if item.factor.profitability >= 60.0 {
        catalysts.push(I18nText::new("pick.catalyst.solid_profitability"));
    }

    // News catalysts
    if !item.news.is_empty() {
        let positive_news = item.news.iter().any(|n| {
            let text = format!("{} {}", n.title, n.summary).to_lowercase();
            [
                "beat",
                "growth",
                "upgrade",
                "approval",
                "expansion",
                "contract",
            ]
            .iter()
            .any(|keyword| text.contains(keyword))
        });
        if positive_news {
            catalysts.push(I18nText::new("pick.catalyst.positive_news"));
        }
    }

    // Valuation catalyst
    if item.factor.value >= 60.0 {
        catalysts.push(I18nText::new("pick.catalyst.attractive_valuation"));
    }

    if catalysts.is_empty() {
        catalysts.push(I18nText::new("pick.catalyst.leading_composite_score"));
    }
    catalysts
}

pub(crate) fn default_risks(item: &EnrichedCandidate) -> Vec<I18nText> {
    let mut risks = Vec::new();

    // Price movement risks
    if item.change_pct.unwrap_or_default() >= 9.5 {
        risks.push(I18nText::new("pick.risk.large_short_term_gain"));
    }
    if item.change_pct.unwrap_or_default() <= -5.0 {
        risks.push(I18nText::new("pick.risk.recent_decline"));
    }

    // Valuation risks
    if item.factor.value < 45.0 {
        risks.push(I18nText::new("pick.risk.average_valuation"));
    }
    if let Some(pe) = item.fundamental_snapshot.pe_like
        && pe > 50.0
    {
        risks.push(I18nText::new("pick.risk.high_pe"));
    }

    // Volatility risks
    if item.factor.risk < 50.0 {
        risks.push(I18nText::new("pick.risk.elevated_volatility"));
    }
    if let Some(atr) = item.technical_snapshot.atr
        && let Some(price) = item.price
        && price > 0.0
    {
        let atr_pct = (atr / price) * 100.0;
        if atr_pct > 5.0 {
            risks.push(I18nText::new("pick.risk.high_atr"));
        }
    }

    // Technical risks
    if let Some(rsi) = item.technical_snapshot.rsi
        && rsi > 70.0
    {
        risks.push(I18nText::new("pick.risk.overbought_rsi"));
    }

    // Negative news risks
    if item.news_snapshot.hard_negative_count > 0 {
        risks.push(I18nText::new("pick.risk.negative_news"));
    }

    if risks.is_empty() {
        risks.push(I18nText::new("pick.risk.continue_tracking"));
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

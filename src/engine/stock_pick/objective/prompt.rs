use std::collections::{HashMap, HashSet};

use crate::engine::stock_pick::EnrichedCandidate;

pub(super) struct IndustryAverages {
    pe_avg: f64,
    ps_avg: f64,
}

pub(super) fn compute_industry_averages(
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

pub(super) fn build_valuation_vs_industry_block(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::stock_pick::FactorBreakdown;
    use crate::models::{
        StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
        StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
        StockPickTechnicalSnapshot,
    };

    fn make_candidate(industry: &str, pe: Option<f64>, ps: Option<f64>) -> EnrichedCandidate {
        EnrichedCandidate {
            symbol: "TEST".to_string(),
            name: "Test Corp".to_string(),
            market: "A-share".to_string(),
            exchange: "CN".to_string(),
            industry: industry.to_string(),
            price: Some(10.0),
            change_pct: Some(1.5),
            market_cap: Some(1_000_000_000.0),
            theme_key: "test".to_string(),
            fundamentals: None,
            news: vec![],
            evidence_records: vec![],
            candles: vec![],
            technical_snapshot: StockPickTechnicalSnapshot::default(),
            market_snapshot: StockPickMarketSnapshot::default(),
            fundamental_snapshot: StockPickFundamentalSnapshot {
                pe_like: pe,
                ps_like: ps,
                ..StockPickFundamentalSnapshot::default()
            },
            news_snapshot: StockPickNewsSnapshot::default(),
            history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
            risk_snapshot: StockPickRiskSnapshot::default(),
            data_quality_snapshot: StockPickDataQualitySnapshot::default(),
            factor: FactorBreakdown {
                total: 70.0,
                momentum: 65.0,
                quality: 60.0,
                value: 55.0,
                profitability: 58.0,
                risk: 62.0,
                event: 50.0,
                evidence: 55.0,
                history: 50.0,
                penalty: 0.0,
            },
            pass_filter: true,
            rejected_reasons: vec![],
            description: String::new(),
        }
    }

    #[test]
    fn test_compute_industry_averages_empty() {
        let result = compute_industry_averages(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_industry_averages_insufficient_data() {
        let candidates = vec![make_candidate("Tech", Some(20.0), Some(5.0))];
        let result = compute_industry_averages(&candidates);
        // Only 1 candidate, needs at least 2
        assert!(result.is_empty());
    }

    #[test]
    fn test_compute_industry_averages_sufficient() {
        let candidates = vec![
            make_candidate("Tech", Some(20.0), Some(5.0)),
            make_candidate("Tech", Some(30.0), Some(8.0)),
        ];
        let result = compute_industry_averages(&candidates);
        assert!(result.contains_key("Tech"));
        let avg = result.get("Tech").unwrap();
        assert!((avg.pe_avg - 25.0).abs() < 0.01);
        assert!((avg.ps_avg - 6.5).abs() < 0.01);
    }

    #[test]
    fn test_compute_industry_averages_skips_unknown() {
        let candidates = vec![
            make_candidate("Unknown", Some(20.0), Some(5.0)),
            make_candidate("Unknown", Some(30.0), Some(8.0)),
        ];
        let result = compute_industry_averages(&candidates);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_valuation_line_none() {
        assert!(format_valuation_line("PE", None, 20.0).is_none());
    }

    #[test]
    fn test_format_valuation_line_premium() {
        let result = format_valuation_line("PE", Some(25.0), 20.0);
        assert!(result.is_some());
        let line = result.unwrap();
        assert!(line.contains("premium"));
        assert!(line.contains("1.3x"));
    }

    #[test]
    fn test_format_valuation_line_discount() {
        let result = format_valuation_line("PE", Some(15.0), 20.0);
        assert!(result.is_some());
        let line = result.unwrap();
        assert!(line.contains("discount"));
    }

    #[test]
    fn test_format_valuation_line_zero_value() {
        assert!(format_valuation_line("PE", Some(0.0), 20.0).is_none());
    }

    #[test]
    fn test_format_valuation_line_negative_value() {
        assert!(format_valuation_line("PE", Some(-5.0), 20.0).is_none());
    }

    #[test]
    fn test_build_valuation_vs_industry_block_empty() {
        let result = build_valuation_vs_industry_block(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_default_thesis() {
        let candidate = make_candidate("Tech", Some(20.0), Some(5.0));
        let thesis = default_thesis(&candidate);
        assert!(thesis.contains("Test Corp"));
        assert!(thesis.contains("70.0"));
    }

    #[test]
    fn test_default_catalysts_strong() {
        let candidate = make_candidate("Tech", Some(20.0), Some(5.0));
        let catalysts = default_catalysts(&candidate);
        // momentum=65 < 70, event=50 < 60, quality=60 >= 60
        assert!(catalysts.iter().any(|c| c.contains("Quality")));
    }

    #[test]
    fn test_default_catalysts_fallback() {
        let mut candidate = make_candidate("Tech", Some(20.0), Some(5.0));
        candidate.factor.momentum = 50.0;
        candidate.factor.event = 50.0;
        candidate.factor.quality = 50.0;
        let catalysts = default_catalysts(&candidate);
        assert!(catalysts.iter().any(|c| c.contains("Composite")));
    }

    #[test]
    fn test_default_risks_high_change() {
        let mut candidate = make_candidate("Tech", Some(20.0), Some(5.0));
        candidate.change_pct = Some(10.0);
        let risks = default_risks(&candidate);
        assert!(risks.iter().any(|r| r.contains("pullback")));
    }

    #[test]
    fn test_default_risks_low_value() {
        let mut candidate = make_candidate("Tech", Some(20.0), Some(5.0));
        candidate.factor.value = 40.0;
        let risks = default_risks(&candidate);
        assert!(risks.iter().any(|r| r.contains("Valuation")));
    }

    #[test]
    fn test_default_risks_fallback() {
        let mut candidate = make_candidate("Tech", Some(20.0), Some(5.0));
        candidate.change_pct = Some(1.0);
        candidate.factor.value = 60.0;
        candidate.factor.risk = 60.0;
        let risks = default_risks(&candidate);
        assert!(risks.iter().any(|r| r.contains("tracking")));
    }

    #[test]
    fn test_default_evidence_with_candles() {
        use crate::data::CandlePoint;
        use rust_decimal::Decimal;
        let mut candidate = make_candidate("Tech", Some(20.0), Some(5.0));
        candidate.candles = vec![
            CandlePoint {
                trade_date: "2024-01-01".to_string(),
                open: Decimal::from(10),
                close: Decimal::from(10),
                high: Decimal::from(11),
                low: Decimal::from(9),
                volume: 1000,
                amount: Decimal::ZERO,
                amplitude_pct: 1.0,
                change_pct: 1.0,
                change_amount: Decimal::from(1),
                turnover_pct: 1.0,
            },
            CandlePoint {
                trade_date: "2024-01-02".to_string(),
                open: Decimal::from(10),
                close: Decimal::from(11),
                high: Decimal::from(12),
                low: Decimal::from(10),
                volume: 2000,
                amount: Decimal::ZERO,
                amplitude_pct: 1.0,
                change_pct: 2.0,
                change_amount: Decimal::from(1),
                turnover_pct: 1.0,
            },
        ];
        let evidence = default_evidence(&candidate);
        assert!(evidence.len() >= 3);
        assert!(evidence.iter().any(|e| e.contains("2024-01-01")));
    }

    #[test]
    fn test_default_evidence_with_news() {
        use crate::data::NewsItem;
        let mut candidate = make_candidate("Tech", Some(20.0), Some(5.0));
        candidate.news = vec![NewsItem {
            published_at: "2024-01-15".to_string(),
            title: "Test".to_string(),
            summary: "".to_string(),
            source: "test".to_string(),
            url: None,
        }];
        let evidence = default_evidence(&candidate);
        assert!(evidence.iter().any(|e| e.contains("news")));
    }
}

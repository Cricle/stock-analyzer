use crate::models::{AnalysisResult, ReferenceFactItem, ReportDiagnosticItem};

pub(crate) fn build_decision_fact_sheet(result: &AnalysisResult) -> String {
    let mut sections = Vec::new();

    let mut overview_lines = Vec::new();
    if result.report.research_confidence_score > 0 {
        overview_lines.push(format!(
            "- Research raw score: {} / 100",
            result.report.research_confidence_score
        ));
    }
    if result.report.confidence_score > 0 {
        overview_lines.push(format!(
            "- Execution confidence score: {} / 100",
            result.report.confidence_score
        ));
    }
    if !result.report.recommendation.trim().is_empty() {
        overview_lines.push(format!(
            "- Current calibrated rating: {}",
            result.report.recommendation.trim()
        ));
    }
    if !result.report.raw_llm_recommendation.trim().is_empty() {
        overview_lines.push(format!(
            "- Current raw rating: {}",
            result.report.raw_llm_recommendation.trim()
        ));
    }
    if !overview_lines.is_empty() {
        sections.push(format!("Decision snapshot:\n{}", overview_lines.join("\n")));
    }

    if !result.report.market_chart.candles.is_empty() {
        let chart = &result.report.market_chart;
        let price = &result.report.price_context;
        let mut lines = vec![format!(
            "- K-line window: {} candles, {} to {}",
            chart.candles.len(),
            chart.start_date,
            chart.end_date
        )];
        if let Some(value) = price.current_price {
            lines.push(format!("- Current price: {value:.4}"));
        }
        if let Some(value) = price.high_price {
            lines.push(format!("- Recent high: {value:.4} on {}", price.high_date));
        }
        if let Some(value) = price.low_price {
            lines.push(format!("- Recent low: {value:.4} on {}", price.low_date));
        }
        if let Some(value) = price.range_pct {
            lines.push(format!("- Lookback range: {value:.2}%"));
        }
        let probability = &result.report.probability_view;
        if probability.upside_probability_pct > 0.0
            || probability.downside_probability_pct > 0.0
            || probability.sideways_probability_pct > 0.0
        {
            lines.push(format!(
                "- Probabilities: upside={:.0}%, downside={:.0}%, sideways={:.0}%, risk={:.0}%",
                probability.upside_probability_pct,
                probability.downside_probability_pct,
                probability.sideways_probability_pct,
                probability.risk_probability_pct,
            ));
        }
        if let Some(value) = result.report.profit_risk.reward_risk_ratio {
            lines.push(format!("- Reward/risk ratio (current→target): {value:.2}"));
        }
        if let Some(value) = result.report.profit_risk.current_position_reward_risk_ratio {
            lines.push(format!("- Reward/risk ratio (current→confirmation): {value:.2}"));
        }
        sections.push(format!(
            "Structured price and probability facts:\n{}",
            lines.join("\n")
        ));
    }

    push_fact_section(
        "Structured market facts",
        &result.report.references.market,
        &mut sections,
    );
    push_fact_section(
        "Structured fundamentals facts",
        &result.report.references.fundamentals,
        &mut sections,
    );
    push_fact_section(
        "Structured news facts",
        &result.report.references.news,
        &mut sections,
    );
    push_fact_section(
        "Historical calibration facts",
        &result.report.references.memory,
        &mut sections,
    );
    push_diagnostic_section(
        "Availability and data limits",
        &result.report.diagnostics.availability,
        &mut sections,
    );
    push_diagnostic_section(
        "Market diagnostics",
        &result.report.diagnostics.market,
        &mut sections,
    );
    push_diagnostic_section(
        "Fundamentals diagnostics",
        &result.report.diagnostics.fundamentals,
        &mut sections,
    );
    push_diagnostic_section(
        "News diagnostics",
        &result.report.diagnostics.news,
        &mut sections,
    );

    sections.join("\n\n")
}

fn push_fact_section(title: &str, items: &[ReferenceFactItem], sections: &mut Vec<String>) {
    if items.is_empty() {
        return;
    }
    let body = items
        .iter()
        .filter(|item| {
            let key = item.key.trim();
            let emphasis = item.emphasis.trim();
            emphasis.eq_ignore_ascii_case("primary")
                || emphasis.eq_ignore_ascii_case("warning")
                || emphasis.eq_ignore_ascii_case("success")
                || matches!(
                    key,
                    "latest_close"
                        | "window_return"
                        | "range_pct"
                        | "ma50"
                        | "ma200"
                        | "ema10"
                        | "macd"
                        | "rsi"
                        | "atr"
                        | "vwap"
                        | "vwma"
                        | "market_cap"
                        | "cash_and_equivalents"
                        | "total_debt"
                        | "current_debt"
                        | "long_term_debt"
                        | "revenues"
                        | "gross_profit"
                        | "operating_income"
                        | "operating_expenses"
                        | "net_income"
                        | "operating_cash_flow"
                        | "free_cash_flow"
                        | "research_raw_score"
                        | "verified_setup_samples"
                        | "setup_hit_rate"
                        | "setup_avg_alpha"
                )
        })
        .take(10)
        .map(|item| {
            let emphasis = if item.emphasis.trim().is_empty() {
                String::new()
            } else {
                format!(" [{}]", item.emphasis.trim())
            };
            format!("- {}: {}{}", item.label.trim(), item.value.trim(), emphasis)
        })
        .collect::<Vec<_>>()
        .join("\n");
    sections.push(format!("{title}:\n{body}"));
}

fn push_diagnostic_section(
    title: &str,
    items: &[ReportDiagnosticItem],
    sections: &mut Vec<String>,
) {
    if items.is_empty() {
        return;
    }
    let body = items
        .iter()
        .filter(|item| {
            matches!(
                item.severity.trim(),
                "warning" | "error" | "critical" | "success" | "info"
            )
        })
        .take(6)
        .map(|item| {
            let mut line = format!(
                "- {} [{}]: {}",
                item.code.trim(),
                item.severity.trim(),
                item.message.trim()
            );
            if !item.details.is_empty() {
                line.push_str(&format!(" ({})", item.details.join("; ")));
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    sections.push(format!("{title}:\n{body}"));
}

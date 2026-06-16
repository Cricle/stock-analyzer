fn score_data_quality(
    non_empty_core: usize,
    analyst_count: usize,
    tool_successes: usize,
    tool_failures: usize,
) -> ScoreDimension {
    let mut score = 0.0_f64;

    // Core analysis desks — sigmoid (0-12 points)
    score += sigmoid(non_empty_core as f64, 3.0, 1.5) * 12.0;

    // Analyst count — sigmoid (0-4 points)
    score += sigmoid(analyst_count as f64, 2.0, 1.5) * 4.0;

    // Tool success rate — sigmoid (0-5 points)
    let total_tools = tool_successes + tool_failures;
    let success_rate = if total_tools > 0 {
        tool_successes as f64 / total_tools as f64
    } else {
        0.5
    };
    score += sigmoid(success_rate, 0.7, 8.0) * 5.0;

    // Tool failure penalty — exponential decay
    score -= exponential_decay(tool_failures as f64, 2.0) * 3.0;

    ScoreDimension {
        score: score.clamp(0.0, DATA_QUALITY_MAX as f64) as i32,
        max_score: DATA_QUALITY_MAX,
        rationale: format!(
            "核心分析台完成数={non_empty_core}/4，结构化分析台数={analyst_count}，成功工具调用={tool_successes}，失败工具调用={tool_failures}。"
        ).into(),
    }
}

fn score_trend_confirmation(
    analyst: Option<&AgentReportNode>,
    market_report: &str,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> ScoreDimension {
    let evidence_points = analyst.map_or(0, |item| item.evidence_points.len());
    let numeric_levels = count_numeric_levels(market_report)
        + trader_plan.entry_price_numeric_count()
        + trader_plan.stop_loss_numeric_count()
        + portfolio_decision.price_target_numeric_count();
    let probability_quality = analyst_probability_quality(analyst);

    let mut score = 0.0_f64;

    // Report presence — sigmoid (0-5 points)
    let report_present = if market_report.trim().is_empty() { 0.0 } else { 1.0 };
    score += report_present * 5.0;

    // Evidence points — sigmoid (0-6 points)
    score += sigmoid(evidence_points as f64, 3.0, 1.0) * 6.0;

    // Numeric anchors — sigmoid (0-6 points)
    score += sigmoid(numeric_levels as f64, 3.0, 1.0) * 6.0;

    // Probability quality — sigmoid (0-3 points)
    score += sigmoid(probability_quality as f64, 3.0, 1.0) * 3.0;

    ScoreDimension {
        score: score.clamp(0.0, TREND_CONFIRMATION_MAX as f64) as i32,
        max_score: TREND_CONFIRMATION_MAX,
        rationale: format!(
            "市场技术报告存在={}，证据点={}，价位/指标数值锚点={}，概率闭合质量={}。",
            bool_text(!market_report.trim().is_empty()),
            evidence_points,
            numeric_levels,
            probability_quality
        ).into(),
    }
}

fn score_fundamentals(
    analyst: Option<&AgentReportNode>,
    fundamentals_report: &str,
) -> ScoreDimension {
    let evidence_points = analyst.map_or(0, |item| item.evidence_points.len());
    let numeric_levels = count_numeric_levels(fundamentals_report);
    let probability_quality = analyst_probability_quality(analyst);

    let mut score = 0.0_f64;

    // Report presence — sigmoid (0-6 points)
    let report_present = if fundamentals_report.trim().is_empty() { 0.0 } else { 1.0 };
    score += report_present * 6.0;

    // Evidence points — sigmoid (0-6 points)
    score += sigmoid(evidence_points as f64, 3.0, 1.0) * 6.0;

    // Numeric anchors — sigmoid (0-6 points)
    score += sigmoid(numeric_levels as f64, 3.0, 1.0) * 6.0;

    // Probability quality — sigmoid (0-1.5 points)
    score += sigmoid(probability_quality as f64, 3.0, 1.0) * 1.5;

    ScoreDimension {
        score: score.clamp(0.0, FUNDAMENTAL_CONFIRMATION_MAX as f64) as i32,
        max_score: FUNDAMENTAL_CONFIRMATION_MAX,
        rationale: format!(
            "基本面报告存在={}，证据点={}，数值锚点={}，概率闭合质量={}。",
            bool_text(!fundamentals_report.trim().is_empty()),
            evidence_points,
            numeric_levels,
            probability_quality
        ).into(),
    }
}

fn score_catalyst_quality(
    analyst: Option<&AgentReportNode>,
    news_report: &str,
    portfolio_decision: &StructuredPortfolioDecision,
) -> ScoreDimension {
    let evidence_points = analyst.map_or(0, |item| item.evidence_points.len());
    let next_steps = analyst.map_or(0, |item| item.next_steps.len());
    let date_hits = count_numeric_dates(news_report);
    let horizon_dates = count_numeric_dates(&portfolio_decision.time_horizon);

    let mut score = 0.0_f64;

    // News report presence — sigmoid (0-4 points)
    let report_present = if news_report.trim().is_empty() { 0.0 } else { 1.0 };
    score += report_present * 4.0;

    // Evidence points — sigmoid (0-5 points)
    score += sigmoid(evidence_points as f64, 3.0, 1.2) * 5.0;

    // Next steps — sigmoid (0-3 points)
    score += sigmoid(next_steps as f64, 2.0, 1.5) * 3.0;

    // Date anchors — sigmoid (0-3 points)
    let total_dates = (date_hits + horizon_dates) as f64;
    score += sigmoid(total_dates, 2.0, 1.5) * 3.0;

    ScoreDimension {
        score: score.clamp(0.0, CATALYST_QUALITY_MAX as f64) as i32,
        max_score: CATALYST_QUALITY_MAX,
        rationale: format!(
            "新闻报告存在={}，证据点={}，后续跟踪项={}，日期/时间线锚点={}。",
            bool_text(!news_report.trim().is_empty()),
            evidence_points,
            next_steps,
            date_hits + horizon_dates
        ).into(),
    }
}

fn score_historical_transferability(result: &AnalysisResult) -> ScoreDimension {
    let memory = &result.artifacts.memory_context;
    let setup_match_count = memory.setup_match_count as f64;
    let setup_resolved_match_count = memory.setup_resolved_match_count as f64;
    let same_ticker_count = memory.same_ticker_count as f64;
    let cross_ticker_count = memory.cross_ticker_count as f64;

    let mut score = 0.0_f64;

    if memory.used_setup_filtered_retrieval {
        // Match bonus multiplier: fallback weakens the signal
        let match_bonus = if memory.used_setup_fallback_calibration { 0.7 } else { 1.0 };

        // Setup match count — sigmoid (0-3 points × bonus)
        score += sigmoid(setup_match_count, 2.0, 1.5) * 3.0 * match_bonus;
        score += sigmoid(setup_resolved_match_count, 2.0, 1.5) * 3.0 * match_bonus;

        // Hit rate — sigmoid (0-2 points × bonus)
        score += sigmoid(memory.setup_match_hit_rate, 0.55, 10.0) * 2.0 * match_bonus;

        // Alpha return — sigmoid (0-2 points × bonus)
        score += sigmoid(memory.setup_match_avg_alpha_return, 0.015, 50.0) * 2.0 * match_bonus;
    } else if same_ticker_count > 0.0 || cross_ticker_count > 0.0 {
        score += 3.0;
    }

    // Same/cross ticker samples — sigmoid
    score += sigmoid(same_ticker_count, 1.0, 2.0) * 1.0;
    score += sigmoid(cross_ticker_count, 2.0, 1.0) * 1.0;

    ScoreDimension {
        score: score.clamp(0.0, HISTORICAL_TRANSFERABILITY_MAX as f64) as i32,
        max_score: HISTORICAL_TRANSFERABILITY_MAX,
        rationale: format!(
            "setup 过滤启用={}，fallback 弱校准={}，setup 命中数={}，已验证命中={}，命中率={:.0}%，平均超额收益={:.1}%，同票样本={}，跨票样本={}。相似历史越充分且结果越稳健，当前结论越具可迁移性。",
            bool_text(memory.used_setup_filtered_retrieval),
            bool_text(memory.used_setup_fallback_calibration),
            memory.setup_match_count,
            memory.setup_resolved_match_count,
            memory.setup_match_hit_rate * 100.0,
            memory.setup_match_avg_alpha_return * 100.0,
            memory.same_ticker_count,
            memory.cross_ticker_count
        ).into(),
    }
}

pub fn score_setup_direction_alignment(result: &AnalysisResult) -> ScoreDimension {
    let memory = &result.artifacts.memory_context;
    let recommendation = &result.structured_portfolio_decision().rating;
    let current_direction = if recommendation.is_bullish() {
        1
    } else if recommendation.is_bearish() {
        -1
    } else {
        0
    };

    let score = if memory.setup_resolved_match_count == 0 || current_direction == 0 {
        4
    } else {
        let (aligned, opposed) = if current_direction > 0 {
            (memory.setup_long_match_count as f64, memory.setup_short_match_count as f64)
        } else {
            (memory.setup_short_match_count as f64, memory.setup_long_match_count as f64)
        };
        let ratio = (aligned - opposed) / memory.setup_resolved_match_count as f64;
        // Continuous mapping: ratio=-0.5→3, ratio=0→6, ratio=0.15→8, ratio=0.4→10
        let base = sigmoid(ratio, 0.1, 8.0) * 7.0 + 3.0;
        base.clamp(3.0, 10.0) as i32
    };

    ScoreDimension {
        score,
        max_score: HISTORICAL_TRANSFERABILITY_MAX,
        rationale: format!(
            "当前方向={}，相似 setup 已验证样本中偏多={}，偏空={}，中性={}。历史方向分布越与当前建议一致，迁移可信度越高。",
            if current_direction > 0 {
                "多头"
            } else if current_direction < 0 {
                "空头"
            } else {
                "中性"
            },
            memory.setup_long_match_count,
            memory.setup_short_match_count,
            memory.setup_neutral_match_count
        ).into(),
    }
}

fn score_cross_agent_consistency(result: &AnalysisResult) -> ScoreDimension {
    let nets = result
        .graph
        .analysts
        .iter()
        .map(|item| item.up_probability - item.down_probability)
        .collect::<Vec<_>>();
    if nets.is_empty() {
        return ScoreDimension {
            score: 6,
            max_score: CROSS_AGENT_CONSISTENCY_MAX,
            rationale: "缺少结构化概率节点，只能给基础分。".into(),
        };
    }

    let positive = nets.iter().filter(|item| **item > 0.05).count();
    let negative = nets.iter().filter(|item| **item < -0.05).count();
    let neutral = nets.len().saturating_sub(positive + negative);

    // Direction consensus — sigmoid (0-10 points)
    let consensus_ratio = (positive.max(negative) as f64) / nets.len() as f64;
    let consensus_score = sigmoid(consensus_ratio, 0.6, 8.0) * 10.0;

    // Direction strength — sigmoid (0-5 points)
    let avg_abs = nets.iter().map(|item| item.abs()).sum::<f64>() / nets.len() as f64;
    let strength_score = sigmoid(avg_abs, 0.15, 15.0) * 5.0;

    let score = (consensus_score + strength_score).clamp(0.0, CROSS_AGENT_CONSISTENCY_MAX as f64) as i32;

    ScoreDimension {
        score,
        max_score: CROSS_AGENT_CONSISTENCY_MAX,
        rationale: format!(
            "偏多={}，偏空={}，中性={}，共识比={:.0}%，平均方向强度={:.2}。",
            positive, negative, neutral, consensus_ratio * 100.0, avg_abs
        ).into(),
    }
}

fn score_risk_clarity(
    result: &AnalysisResult,
    research_plan: &StructuredResearchPlan,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> ScoreDimension {
    let risk_turns = result.graph.risk_debate.turns.len();
    let numeric_levels = trader_plan.stop_loss_numeric_count()
        + portfolio_decision.price_target_numeric_count()
        + count_numeric_levels(portfolio_decision.risk_assessment.as_str())
        + count_numeric_levels(research_plan.risk_assessment.as_str());
    let exec_boundary = has_execution_boundary(trader_plan, portfolio_decision);

    let mut score = 0.0_f64;

    // Risk conclusion presence — sigmoid (0-3 points)
    let risk_present = if !research_plan.risk_assessment.trim().is_empty()
        || !portfolio_decision.risk_assessment.trim().is_empty()
    { 1.0 } else { 0.0 };
    score += risk_present * 3.0;

    // Risk debate turns — sigmoid (0-3 points)
    score += sigmoid(risk_turns as f64, 2.0, 1.5) * 3.0;

    // Numeric risk control boundaries — sigmoid (0-3 points)
    score += sigmoid(numeric_levels as f64, 2.0, 1.5) * 3.0;

    // Execution boundary completeness — partial credit for entry+stop without full plan
    score += match exec_boundary {
        ExecutionBoundaryLevel::Complete => 1.0,
        ExecutionBoundaryLevel::Partial => 0.5,
        ExecutionBoundaryLevel::Missing => 0.0,
    };

    let exec_label = match exec_boundary {
        ExecutionBoundaryLevel::Complete => "common.yes",
        ExecutionBoundaryLevel::Partial => "common.partial",
        ExecutionBoundaryLevel::Missing => "common.no",
    };
    ScoreDimension {
        score: score.clamp(0.0, RISK_CLARITY_MAX as f64) as i32,
        max_score: RISK_CLARITY_MAX,
        rationale: format!(
            "风险结论存在={}，风险辩论轮次={}，数值风控边界={}，执行边界完整={}。",
            bool_text(
                !research_plan.risk_assessment.trim().is_empty()
                    || !portfolio_decision.risk_assessment.trim().is_empty()
            ),
            risk_turns,
            numeric_levels,
            exec_label
        ).into(),
    }
}

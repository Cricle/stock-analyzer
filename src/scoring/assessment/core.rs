pub fn score_data_quality(
    non_empty_core: usize,
    analyst_count: usize,
    tool_successes: usize,
    tool_failures: usize,
) -> ScoreDimension {
    let mut score = 0;
    score += (non_empty_core as i32 * 3).min(12);
    score += analyst_count.min(4) as i32;
    score += tool_successes.min(5) as i32;
    score -= (tool_failures as i32 * 2).min(6);
    ScoreDimension {
        score: score.clamp(0, DATA_QUALITY_MAX),
        max_score: DATA_QUALITY_MAX,
        rationale: format!(
            "核心分析台完成数={non_empty_core}/4，结构化分析台数={analyst_count}，成功工具调用={tool_successes}，失败工具调用={tool_failures}。"
        ).into(),
    }
}

pub fn score_trend_confirmation(
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

    let mut score = if market_report.trim().is_empty() {
        0
    } else {
        5
    };
    score += evidence_points.min(12) as i32;
    score += numeric_levels.min(12);
    score += probability_quality;

    ScoreDimension {
        score: score.clamp(0, TREND_CONFIRMATION_MAX),
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

pub fn score_fundamentals(
    analyst: Option<&AgentReportNode>,
    fundamentals_report: &str,
) -> ScoreDimension {
    let evidence_points = analyst.map_or(0, |item| item.evidence_points.len());
    let numeric_levels = count_numeric_levels(fundamentals_report);
    let probability_quality = analyst_probability_quality(analyst);

    let mut score = if fundamentals_report.trim().is_empty() {
        0
    } else {
        6
    };
    score += evidence_points.min(12) as i32;
    score += numeric_levels.min(12);
    score += (probability_quality / 2).max(0);

    ScoreDimension {
        score: score.clamp(0, FUNDAMENTAL_CONFIRMATION_MAX),
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

pub fn score_catalyst_quality(
    analyst: Option<&AgentReportNode>,
    news_report: &str,
    portfolio_decision: &StructuredPortfolioDecision,
) -> ScoreDimension {
    let evidence_points = analyst.map_or(0, |item| item.evidence_points.len());
    let next_steps = analyst.map_or(0, |item| item.next_steps.len());
    let date_hits = count_numeric_dates(news_report);
    let horizon_dates = count_numeric_dates(&portfolio_decision.time_horizon);

    let mut score = if news_report.trim().is_empty() { 0 } else { 5 };
    score += evidence_points.min(8) as i32;
    score += next_steps.min(5) as i32;
    score += (date_hits + horizon_dates).min(5);

    ScoreDimension {
        score: score.clamp(0, CATALYST_QUALITY_MAX),
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

pub fn score_historical_transferability(result: &AnalysisResult) -> ScoreDimension {
    let memory = &result.artifacts.memory_context;
    let setup_match_count = memory.setup_match_count as i32;
    let setup_resolved_match_count = memory.setup_resolved_match_count as i32;
    let same_ticker_count = memory.same_ticker_count as i32;
    let cross_ticker_count = memory.cross_ticker_count as i32;
    let used_setup_filter = memory.used_setup_filtered_retrieval;
    let used_fallback = memory.used_setup_fallback_calibration;

    let mut score = 0;
    if used_setup_filter {
        if used_fallback {
            score += setup_match_count.min(2);
            score += setup_resolved_match_count.min(2);
            if memory.setup_match_hit_rate >= 0.6 {
                score += 1;
            }
            if memory.setup_match_avg_alpha_return > 0.03 {
                score += 1;
            }
        } else {
            score += setup_match_count.min(3);
            score += setup_resolved_match_count.min(3);
            if memory.setup_match_hit_rate >= 0.6 {
                score += 2;
            } else if memory.setup_match_hit_rate >= 0.5 {
                score += 1;
            }
            if memory.setup_match_avg_alpha_return > 0.03 {
                score += 2;
            } else if memory.setup_match_avg_alpha_return > 0.0 {
                score += 1;
            }
        }
    } else if same_ticker_count > 0 || cross_ticker_count > 0 {
        score += 3;
    }
    if same_ticker_count > 0 {
        score += 1;
    }
    if cross_ticker_count >= 2 {
        score += 1;
    }

    ScoreDimension {
        score: score.clamp(0, HISTORICAL_TRANSFERABILITY_MAX),
        max_score: HISTORICAL_TRANSFERABILITY_MAX,
        rationale: format!(
            "setup 过滤启用={}，fallback 弱校准={}，setup 命中数={}，已验证命中={}，命中率={:.0}%，平均超额收益={:.1}%，同票样本={}，跨票样本={}。相似历史越充分且结果越稳健，当前结论越具可迁移性。",
            bool_text(used_setup_filter),
            bool_text(used_fallback),
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
    } else if current_direction > 0 {
        let aligned = memory.setup_long_match_count as f64;
        let opposed = memory.setup_short_match_count as f64;
        let ratio = (aligned - opposed) / memory.setup_resolved_match_count as f64;
        if ratio >= 0.4 {
            10
        } else if ratio >= 0.15 {
            8
        } else if ratio > -0.15 {
            6
        } else {
            3
        }
    } else {
        let aligned = memory.setup_short_match_count as f64;
        let opposed = memory.setup_long_match_count as f64;
        let ratio = (aligned - opposed) / memory.setup_resolved_match_count as f64;
        if ratio >= 0.4 {
            10
        } else if ratio >= 0.15 {
            8
        } else if ratio > -0.15 {
            6
        } else {
            3
        }
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

pub fn score_cross_agent_consistency(result: &AnalysisResult) -> ScoreDimension {
    let nets = result
        .graph
        .analysts
        .iter()
        .map(|item| item.up_probability - item.down_probability)
        .collect::<Vec<_>>();
    if nets.is_empty() {
        return ScoreDimension {
            score: 8,
            max_score: CROSS_AGENT_CONSISTENCY_MAX,
            rationale: "缺少结构化概率节点，只能给基础分。".into(),
        };
    }

    let positive = nets.iter().filter(|item| **item > 0.05).count();
    let negative = nets.iter().filter(|item| **item < -0.05).count();
    let neutral = nets.len().saturating_sub(positive + negative);
    let avg_abs = nets.iter().map(|item| item.abs()).sum::<f64>() / nets.len() as f64;

    let score = if positive == nets.len() || negative == nets.len() {
        if avg_abs >= 0.20 { 25 } else { 22 }
    } else if positive == 0 || negative == 0 {
        if avg_abs >= 0.12 { 18 } else { 15 }
    } else if neutral > 0 {
        12
    } else {
        8
    };

    ScoreDimension {
        score,
        max_score: CROSS_AGENT_CONSISTENCY_MAX,
        rationale: format!(
            "偏多={}，偏空={}，中性={}，平均方向强度={:.2}。",
            positive, negative, neutral, avg_abs
        ).into(),
    }
}

pub fn score_risk_clarity(
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
    let has_execution_boundary = has_execution_boundary(trader_plan, portfolio_decision);

    let mut score = 0;
    if !research_plan.risk_assessment.trim().is_empty()
        || !portfolio_decision.risk_assessment.trim().is_empty()
    {
        score += 3;
    }
    score += risk_turns.min(3) as i32;
    score += numeric_levels.min(3);
    if has_execution_boundary {
        score += 1;
    }

    ScoreDimension {
        score: score.clamp(0, RISK_CLARITY_MAX),
        max_score: RISK_CLARITY_MAX,
        rationale: format!(
            "风险结论存在={}，风险辩论轮次={}，数值风控边界={}，执行边界完整={}。",
            bool_text(
                !research_plan.risk_assessment.trim().is_empty()
                    || !portfolio_decision.risk_assessment.trim().is_empty()
            ),
            risk_turns,
            numeric_levels,
            bool_text(has_execution_boundary)
        ).into(),
    }
}

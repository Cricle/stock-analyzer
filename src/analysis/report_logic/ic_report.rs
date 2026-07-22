
/// Parameters for [`build_overview_section`].
pub(crate) struct OverviewSectionParams<'a> {
    pub result: &'a AnalysisResult,
    pub portfolio_decision: &'a StructuredPortfolioDecision,
    pub recommendation: &'a str,
    pub confidence_score: i32,
    pub research_confidence_score: i32,
    pub research_reliability: &'a ResearchReliability,
    pub core_research_call: &'a CoreResearchCall,
    pub decision_view: &'a DecisionView,
    pub decision_narrative: &'a str,
    pub mispricing_claim: &'a LocalText,
    pub why_now: &'a LocalText,
    pub required_confirmation: &'a LocalText,
    pub max_initial_risk_budget: &'a LocalText,
}

fn build_overview_section(
    params: OverviewSectionParams<'_>,
) -> Option<ReportSection> {
    let summary = first_non_empty_sentence(&[
        params.portfolio_decision.executive_summary.as_str(),
        params.portfolio_decision.investment_thesis.as_str(),
        params.portfolio_decision.rationale.as_str(),
        params.result.derived_summary().as_str(),
    ]);
    let risk = first_non_empty_sentence(&[
        params.portfolio_decision.risk_assessment.as_str(),
        params.result.derived_risk_assessment().as_str(),
    ]);
    let rationale = first_non_empty_sentence(&[params.result.derived_rationale().as_str()]);

    let mut lines = Vec::new();
    let display_rating = if params.recommendation.trim().is_empty() {
        "Hold"
    } else {
        params.recommendation.trim()
    };
    lines.push(format!("- 核心研究结论: **{}**", params.core_research_call));
    lines.push(format!("- 执行动作: **{}**", decision_action_code(&params.decision_view.action)));
    lines.push(format!("- 执行评级: **{}**", display_rating));
    lines.push(format!("- 决策解释: {}", params.decision_narrative));
    lines.push(format!(
        "- 研究可靠性: **{} / {}**（{}）",
        params.research_reliability.score, params.research_reliability.max_score, params.research_reliability.label.key
    ));
    lines.push(format!("- 研究原始分: **{} / 100**", params.research_confidence_score));
    lines.push(format!("- 当前可执行把握: **{} / 100**", params.confidence_score));
    if params.research_reliability.score >= 75 && params.confidence_score <= 35 {
        lines.push(
            "- 说明: 分数偏低主要反映执行时点和历史迁移把握不足，不代表研究本身失真。"
                .to_string(),
        );
    }
    if let Some(summary) = summary.as_ref() {
        lines.push(format!("- 核心结论: {}", summary));
    }
    if !params.mispricing_claim.key.is_empty() {
        lines.push(format!("- 错价主张: {}", params.mispricing_claim.key));
    }
    if !params.why_now.key.is_empty() {
        lines.push(format!("- 为什么是现在: {}", params.why_now.key));
    }
    if !params.required_confirmation.key.is_empty() {
        lines.push(format!("- 还缺的确认: {}", params.required_confirmation.key));
    }
    if !params.max_initial_risk_budget.key.is_empty() {
        lines.push(format!("- 初始风险预算: {}", params.max_initial_risk_budget.key));
    }
    lines.push("- 治理约束: 历史 setup 在这一轮只用于约束仓位和升级门槛，不主导主叙事。".to_string());
    if let Some(risk) = risk {
        lines.push(format!("- 主要风险: {}", risk));
    }
    if let Some(rationale) =
        rationale.filter(|item| !is_semantically_similar(Some(item), summary.as_ref()))
    {
        lines.push(format!("- 当前依据: {}", rationale));
    }
    lines.push(format!(
        "- 已完成阶段: {}",
        summarize_stage_state(&params.result.report_stage())
    ));

    (!lines.is_empty()).then(|| ReportSection {
        key: "overview".to_string(),
        title: "总览".to_string(),
        content: lines.join("\n"),
    })
}

fn decision_action_code(action: &DecisionAction) -> &'static str {
    match action {
        DecisionAction::BuyNow => "buy_now",
        DecisionAction::ProbePosition => "probe_position",
        DecisionAction::WaitBreakout => "wait_breakout",
        DecisionAction::WaitBreakdown => "wait_breakdown",
        DecisionAction::WaitRetest => "wait_retest",
        DecisionAction::Hold => "hold",
        DecisionAction::Reduce => "reduce",
        DecisionAction::Exit => "exit",
    }
}

fn describe_core_research_call(call: &CoreResearchCall) -> &'static str {
    match call {
        CoreResearchCall::LeanBuy => "偏多",
        CoreResearchCall::BuyOnConfirmation => "条件确认后偏多",
        CoreResearchCall::Neutral => "中性观察",
        CoreResearchCall::LeanSell => "偏空",
        CoreResearchCall::SellOnBreak => "破位转空",
    }
}

fn describe_decision_action(action: &DecisionAction) -> &'static str {
    match action {
        DecisionAction::BuyNow => "立即执行买入",
        DecisionAction::ProbePosition => "小仓试探",
        DecisionAction::WaitBreakout => "等待突破确认",
        DecisionAction::WaitBreakdown => "等待跌破确认",
        DecisionAction::WaitRetest => "等待回踩确认",
        DecisionAction::Hold => "继续持有/观察",
        DecisionAction::Reduce => "减仓防守",
        DecisionAction::Exit => "退出",
    }
}

fn describe_execution_state(state: &DecisionExecutionState) -> &'static str {
    match state {
        DecisionExecutionState::Ready => "已可执行",
        DecisionExecutionState::Conditional => "条件待确认",
        DecisionExecutionState::Watchlist => "观察名单",
        DecisionExecutionState::Blocked => "执行受阻",
    }
}

fn build_ic_report_summary(report: &StructuredReport) -> String {
    let main_path = preferred_scenario_path_with_direction(&report.action_guides, Some(&report.decision_view.tilt))
        .map(|path| path.name.clone())
        .unwrap_or_else(|| LocalText::new("ic_waiting_for_confirmation"));
    let research_call = describe_core_research_call(&report.decision_view.tilt);
    let action = describe_decision_action(&report.decision_view.action);
    let execution_state = describe_execution_state(&report.decision_view.execution_state);
    format!(
        "当前主席层判断为 {research_call}，执行状态是{execution_state}，下一步动作是{action}。核心不是否定长期逻辑，而是承认眼下最该执行的主路径是\u{201c}{main_path}\u{201d}，在确认到位前不扩大风险暴露。"
    )
}

fn build_primary_path_call(
    core_research_call: &CoreResearchCall,
    guides: &ReportActionGuides,
    confidence_score: i32,
) -> LocalText {
    let Some(path) = preferred_scenario_path_with_direction(guides, Some(core_research_call)) else {
        return LocalText::default();
    };
    let path_name = path.name.trim().to_string();
    match core_research_call {
        CoreResearchCall::LeanBuy | CoreResearchCall::BuyOnConfirmation => {
            let probability_band = if confidence_score >= 60 { "high" } else if confidence_score >= 45 { "medium" } else { "low" };
            LocalText::new("primary_path_call_bullish")
                .with_str("path_name", path_name)
                .with_str("probability_band", probability_band)
        }
        CoreResearchCall::LeanSell | CoreResearchCall::SellOnBreak => {
            LocalText::new("primary_path_call_bearish").with_str("path_name", path_name)
        }
        CoreResearchCall::Neutral => {
            LocalText::new("primary_path_call_neutral").with_str("path_name", path_name)
        }
    }
}

fn build_path_bias_rationale(
    core_research_call: &CoreResearchCall,
    guides: &ReportActionGuides,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    confidence_score: i32,
) -> LocalText {
    let preferred = preferred_scenario_path_with_direction(guides, Some(core_research_call))
        .map(|path| path.name.trim().to_string())
        .unwrap_or_default();
    let confirmation = visible_confirmation_reference(portfolio_decision)
        .unwrap_or_default();
    let invalidation = visible_invalidation_reference(portfolio_decision, Some(trader_plan))
        .unwrap_or_default();
    match core_research_call {
        CoreResearchCall::LeanBuy | CoreResearchCall::BuyOnConfirmation => {
            LocalText::new("path_bias_bullish")
                .with_str("path_name", preferred)
                .with_str("confirmation", confirmation)
        }
        CoreResearchCall::LeanSell | CoreResearchCall::SellOnBreak => {
            LocalText::new("path_bias_bearish").with_str("path_name", preferred)
        }
        CoreResearchCall::Neutral => {
            let confidence_band = if confidence_score >= 45 { "moderate" } else { "weak" };
            LocalText::new("path_bias_neutral")
                .with_str("confidence_band", confidence_band)
                .with_str("confirmation", confirmation)
                .with_str("invalidation", invalidation)
        }
    }
}

fn build_advance_probe_opinion(
    core_research_call: &CoreResearchCall,
    action: &DecisionAction,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    confidence_score: i32,
    forced_hold: bool,
) -> LocalText {
    if forced_hold {
        return LocalText::new("advance_probe_forced_hold");
    }
    let confirmation = visible_confirmation_reference(portfolio_decision)
        .unwrap_or_default();
    let entry = trader_plan.entry_price.trim().to_string();
    match action {
        DecisionAction::ProbePosition => {
            LocalText::new("advance_probe_position")
                .with_str("entry", entry)
                .with_str("confirmation", confirmation)
        }
        DecisionAction::WaitBreakout => match core_research_call {
            CoreResearchCall::LeanBuy | CoreResearchCall::BuyOnConfirmation if confidence_score >= 45 => {
                LocalText::new("advance_probe_wait_breakout_conditional").with_str("confirmation", confirmation)
            }
            _ => {
                LocalText::new("advance_probe_wait_breakout_strict").with_str("confirmation", confirmation)
            }
        },
        _ => LocalText::new("advance_probe_default"),
    }
}

fn build_abort_plan(
    action: &DecisionAction,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> LocalText {
    let invalidation = visible_invalidation_reference(portfolio_decision, Some(trader_plan))
        .unwrap_or_else(|| trader_plan.stop_loss.trim().to_string());
    if invalidation.trim().is_empty() {
        return LocalText::new("abort_plan_generic");
    }
    match action {
        DecisionAction::ProbePosition | DecisionAction::BuyNow => {
            LocalText::new("abort_plan_probe_or_buy").with_str("invalidation", invalidation)
        }
        _ => {
            LocalText::new("abort_plan_default").with_str("invalidation", invalidation)
        }
    }
}

fn build_ic_report_sections(result: &AnalysisResult, report: &StructuredReport) -> Vec<ReportSection> {
    let preferred_path = preferred_scenario_path_with_direction(&report.action_guides, Some(&report.decision_view.tilt)).cloned();
    let alternate_paths = all_scenario_paths(&report.action_guides)
        .into_iter()
        .filter(|path| preferred_path.as_ref().is_none_or(|current| current.name != path.name))
        .collect::<Vec<_>>();
    let validated_target = visible_target_reference(&report.portfolio_decision);
    let confirmation = visible_confirmation_reference(&report.portfolio_decision);
    let setup_summary = report.calibration_summary.setup_match_explanation.summary.trim();
    let execution_state = if report.execution_readiness.execution_boundary_complete {
        "执行边界基本完整，但仍需按路径条件行动。"
    } else {
        "执行边界还未闭环，这决定了当前不能把方向判断直接升级成主动加风险。"
    };
    let decision_tension = [
        format!(
            "{} 在 {} 的核心张力，不是公司好坏，而是强趋势、拥挤交易、事件窗口和执行赔率是否同时允许现在承担更多风险。",
            result.symbol, result.analysis_date
        ),
        first_non_empty_sentence(&[
            report.summary.as_str(),
            report.portfolio_decision.investment_thesis.as_str(),
            report.portfolio_decision.rationale.as_str(),
        ])
        .unwrap_or_default(),
    ]
    .into_iter()
    .filter(|item| !item.trim().is_empty())
    .map(|item| format!("- {item}"))
    .collect::<Vec<_>>()
    .join("\n");

    let current_judgement = {
        let mut lines = vec![format!(
            "- 当前主判断: **{}**。长期逻辑没有被否定，但当前不是无条件提高风险预算的时点。",
            fallback_rating(&report.portfolio_decision)
        )];
        // Surface the code-computed reward-risk ratio as the authoritative value.
        // This prevents the LLM from generating its own conflicting ratio in
        // reasoning text (e.g. 0.13 when the computed value is 4.98).
        if let Some(rr) = report.profit_risk.reward_risk_ratio {
            let rr_label = crate::analysis::rr_label(rr);
            let current_rr = report.profit_risk.current_position_reward_risk_ratio;
            if let Some(crr) = current_rr
                && (crr - rr).abs() > 0.01
            {
                let crr_label = crate::analysis::rr_label(crr);
                lines.push(format!(
                    "- 系统计算盈亏比（当前价位→确认位）: **{:.2}**（{}）；盈亏比（当前价位→目标位）: **{:.2}**（{}）。两者区别：前者是当前价格到确认突破位的赔率，后者是突破后到目标位的赔率。以代码计算值为准。",
                    crr, crr_label, rr, rr_label
                ));
            } else {
                lines.push(format!(
                    "- 系统计算盈亏比: **{:.2}**（{}）。以代码计算值为准，不要自行推导。",
                    rr, rr_label
                ));
            }
        }
        if let Some(path) = preferred_path.as_ref() {
            lines.push(format!("- 当前采纳路径: **{}**。", path.name));
            lines.push(format!("- 采纳理由: {}", path.trigger.key));
            lines.push(format!("- 当前动作: {}", path.action.key));
        }
        if let Some(level) = confirmation.as_ref() {
            lines.push(format!("- 当前更值得盯住的确认位: {level}"));
        }
        if let Some(level) = validated_target.as_ref() {
            lines.push(format!("- 若趋势继续扩展，可接受的目标/兑现参考位: {level}"));
        }
        lines.join("\n")
    };

    let scenario_section = {
        let mut lines = Vec::new();
        if let Some(path) = preferred_path.as_ref() {
            lines.push(format!("### 当前主路径: {}", path.name));
            lines.push(format!("- 触发条件: {}", path.trigger.key));
            lines.push(format!("- 对应动作: {}", path.action.key));
            lines.push(format!("- 风险边界: {}", path.risk_boundary.key));
            if !path.position_sizing.key.is_empty() {
                lines.push(format!("- 仓位建议: {}", path.position_sizing.key));
            }
            if !path.stop_level.key.is_empty() {
                lines.push(format!("- 止损/止盈: {}", path.stop_level.key));
            }
        }
        for path in alternate_paths {
            lines.push(String::new());
            lines.push(format!("### 备选但未采纳: {}", path.name));
            lines.push(format!("- 触发条件: {}", path.trigger.key));
            lines.push(format!("- 若成立时的动作: {}", path.action.key));
            lines.push(format!("- 当前未采纳原因: {}", path.risk_boundary.key));
            if !path.position_sizing.key.is_empty() {
                lines.push(format!("- 仓位建议: {}", path.position_sizing.key));
            }
            if !path.stop_level.key.is_empty() {
                lines.push(format!("- 止损/止盈: {}", path.stop_level.key));
            }
        }
        lines.join("\n")
    };

    let system_conservatism = [
        format!("- 当前系统之所以保守，是因为{}。", execution_state),
        format!(
            "- 历史 setup 在这一轮只作为治理约束，而不是主结论引擎。当前历史读数: {}",
            if setup_summary.is_empty() { "暂无强历史背书" } else { setup_summary }
        ),
        "- 这意味着系统不会因为少量正面样本就自动升级动作，但也不应该让历史中性样本主导整篇结论。".to_string(),
    ]
    .join("\n");

    let override_questions = [
        "- 这次机会是否具备打破历史归纳的范式转换特征，而不是普通高位强势股？".to_string(),
        "- 如果只允许承担小仓位风险，当前的赔率是否已经值得进行条件化试探？".to_string(),
        "- 哪一条新增证据一旦出现，应该让委员会立刻从等待切换到执行？".to_string(),
        "- 如果当前不出手，真正承担的机会成本是什么？".to_string(),
    ]
    .join("\n");

    let execution_discipline = {
        let mut lines = Vec::new();
        for item in report.portfolio_decision.trigger_checklist.iter().take(4) {
            lines.push(format!("- 升级条件: {item}"));
        }
        for item in report
            .portfolio_decision
            .missing_evidence_ladder
            .blocking_gaps
            .iter()
            .take(3)
        {
            lines.push(format!("- 必须重审而不是继续等待的缺口: {item}"));
        }
        // Time-based stop-loss
        if !report.trader_plan.time_stop_deadline.trim().is_empty() {
            lines.push(format!("- 时间止损: {}", report.trader_plan.time_stop_deadline));
            if !report.trader_plan.time_stop_reason.trim().is_empty() {
                lines.push(format!("- 时间止损触发后: {}", report.trader_plan.time_stop_reason));
            }
        }
        if lines.is_empty() {
            lines.push("- 当前没有足够结构化触发器，意味着下一次重审必须围绕关键位、事件兑现和风险边界展开。".to_string());
        }
        lines.join("\n")
    };

    // Catalyst scoring card section
    let catalyst_section = {
        let card = &report.catalyst_score_card;
        let mut lines = Vec::new();
        if !card.event_name.trim().is_empty() {
            lines.push(format!("### 催化剂评估: {}", card.event_name));
            for item in &card.items {
                let mark = if item.score > 0 { "✓" } else { "✗" };
                lines.push(format!("- [{}] {}", mark, item.question));
                if !item.evidence.trim().is_empty() {
                    lines.push(format!("  证据: {}", item.evidence));
                }
            }
            lines.push(format!("总分: **{} / {}**", card.total_score, card.max_score));
            if !card.interpretation.trim().is_empty() {
                lines.push(format!("解读: {}", card.interpretation));
            }
            if !card.recommended_action.trim().is_empty() {
                lines.push(format!("建议动作: {}", card.recommended_action));
            }
        } else {
            lines.push("暂无催化剂评分卡。当关键事件（如业绩说明会、财报发布）临近时，系统会自动生成评估框架。".to_string());
        }
        lines.join("\n")
    };

    // Review checklist section
    let review_section = {
        let checklist = &report.review_checklist;
        let mut lines = Vec::new();
        if !checklist.daily.is_empty() || !checklist.weekly.is_empty() {
            if !checklist.daily.is_empty() {
                lines.push("### 每日复核（收盘后）".to_string());
                for item in &checklist.daily {
                    lines.push(format!("- [{}] {}", item.category, item.check));
                }
            }
            if !checklist.weekly.is_empty() {
                lines.push("### 每周复核（周末）".to_string());
                for item in &checklist.weekly {
                    lines.push(format!("- [{}] {}", item.category, item.check));
                }
            }
        } else {
            lines.push("每日复核: 关注价格是否接近确认位或失效位，成交量变化。".to_string());
            lines.push("每周复核: 检查技术指标趋势、基本面边际变化、纪律执行情况。".to_string());
        }
        lines.join("\n")
    };

    vec![
        ReportSection {
            key: "ic_decision_tension".to_string(),
            title: "决策张力".to_string(),
            content: decision_tension,
        },
        ReportSection {
            key: "ic_current_judgement".to_string(),
            title: "当前主判断".to_string(),
            content: current_judgement,
        },
        ReportSection {
            key: "ic_scenario_paths".to_string(),
            title: "三条剧情路径".to_string(),
            content: scenario_section,
        },
        ReportSection {
            key: "ic_system_conservatism".to_string(),
            title: "系统为何保守".to_string(),
            content: system_conservatism,
        },
        ReportSection {
            key: "ic_override_questions".to_string(),
            title: "主席层 Override 问题".to_string(),
            content: override_questions,
        },
        ReportSection {
            key: "ic_execution_discipline".to_string(),
            title: "执行与复核纪律".to_string(),
            content: execution_discipline,
        },
        ReportSection {
            key: "ic_catalyst_scoring".to_string(),
            title: "催化剂评分".to_string(),
            content: catalyst_section,
        },
        ReportSection {
            key: "ic_review_checklist".to_string(),
            title: "复核清单".to_string(),
            content: review_section,
        },
    ]
}

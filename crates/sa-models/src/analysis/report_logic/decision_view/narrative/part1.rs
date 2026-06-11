
fn hold_language_implies_buy_on_confirmation(
    research_plan: &StructuredResearchPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> bool {
    if primary_research_rating(research_plan, "", portfolio_decision) != Rating::Hold
        || portfolio_decision.confirmation_level.trim().is_empty()
    {
        return false;
    }

    let combined = [
        research_plan.rationale.as_str(),
        portfolio_decision.executive_summary.as_str(),
        portfolio_decision.investment_thesis.as_str(),
        portfolio_decision.rationale.as_str(),
        portfolio_decision.risk_assessment.as_str(),
    ]
    .join(" ")
    .to_lowercase();

    let has_confirmation_language = [
        "等待确认",
        "等待更优确认",
        "等待确认门槛",
        "条件式跟踪",
        "条件性偏多",
        "不追价",
        "确认后",
        "站稳",
        "突破",
        "回踩确认",
        "等待交易确认",
        "等待更优确认再行动",
        "升级为buy",
        "升级为overweight",
    ]
    .iter()
    .any(|needle| combined.contains(needle));
    let has_constructive_language = [
        "不该悲观",
        "还不该激进",
        "公司质量没有被破坏",
        "修复结构成立",
        "长期逻辑依然扎实",
        "应当等待",
        "中期框架仍偏多",
        "方向偏正",
        "高质量基本面支撑",
        "中期修复标的",
        "不是应当规避的资产",
        "多头赢在长期质量",
        "修复向重估过渡",
    ]
    .iter()
    .any(|needle| combined.contains(needle));

    has_confirmation_language && has_constructive_language
}

fn primary_research_rating(
    research_plan: &StructuredResearchPlan,
    raw_llm_recommendation: &str,
    portfolio_decision: &StructuredPortfolioDecision,
) -> Rating {
    if !portfolio_decision.raw_rating.trim().is_empty() {
        Rating::parse(&portfolio_decision.raw_rating)
    } else if !research_plan.recommendation.trim().is_empty() {
        Rating::parse(research_plan.recommendation.as_str())
    } else if !raw_llm_recommendation.trim().is_empty() {
        Rating::parse(raw_llm_recommendation)
    } else {
        fallback_rating(portfolio_decision)
    }
}

fn derive_mispricing_claim(
    raw_llm_recommendation: &str,
    portfolio_decision: &StructuredPortfolioDecision,
    research_reliability: &ResearchReliability,
) -> LocalText {
    let rating = fallback_rating(portfolio_decision);
    if rating.is_bullish() {
        return LocalText::new("mispricing_claim_bullish");
    }
    if rating.is_bearish() {
        return LocalText::new("mispricing_claim_bearish");
    }
    if raw_llm_recommendation.trim().eq_ignore_ascii_case("Buy") && research_reliability.score >= 70 {
        return LocalText::new("mispricing_claim_buy_signal");
    }
    LocalText::new("mispricing_claim_neutral")
}

fn derive_why_now(
    decision_view: &DecisionView,
    portfolio_decision: &StructuredPortfolioDecision,
) -> LocalText {
    if !decision_view.confirmation_level.trim().is_empty() {
        return LocalText::new("why_now_confirmation")
            .with_str("confirmation", decision_view.confirmation_level.trim());
    }
    if !portfolio_decision.time_horizon.trim().is_empty() {
        return LocalText::new("why_now_time_horizon")
            .with_str("horizon", portfolio_decision.time_horizon.trim());
    }
    LocalText::new("why_now_generic")
}

fn derive_required_confirmation(
    _decision_view: &DecisionView,
    portfolio_decision: &StructuredPortfolioDecision,
) -> LocalText {
    if !portfolio_decision.confirmation_level.trim().is_empty() {
        let confirmation_level = visible_confirmation_reference(portfolio_decision)
            .unwrap_or_else(|| normalize_level_phrase(portfolio_decision.confirmation_level.trim()));
        return LocalText::new("required_confirmation_with_level")
            .with_str("level", confirmation_level);
    }
    LocalText::new("required_confirmation_generic")
}

fn derive_max_initial_risk_budget(
    decision_view: &DecisionView,
    confidence_caps: &[ConfidenceCap],
    memory_threshold_tightened: bool,
) -> LocalText {
    if matches!(decision_view.tilt, CoreResearchCall::Neutral) {
        return LocalText::new("risk_budget_neutral");
    }
    if memory_threshold_tightened
        || confidence_caps.iter().any(|cap| {
            matches!(cap.key.as_str(), "thin_setup_history" | "zero_resolved_setup_history" | "execution_boundary_missing")
        })
    {
        return LocalText::new("risk_budget_constrained");
    }
    if matches!(decision_view.execution_state, DecisionExecutionState::Conditional | DecisionExecutionState::Watchlist) {
        return LocalText::new("risk_budget_conditional");
    }
    LocalText::new("risk_budget_standard")
}

fn derive_reliability_appendix_summary(
    research_reliability: &ResearchReliability,
    memory_context: &MemoryContextSnapshot,
) -> String {
    format!(
        "研究可靠度={} / {}；历史已验证 setup={}，待验证 setup={}，命中率约 {:.0}%。",
        research_reliability.score,
        research_reliability.max_score,
        memory_context.setup_resolved_match_count,
        memory_context.setup_pending_match_count,
        memory_context.setup_match_hit_rate * 100.0
    )
}

fn build_decision_state_line(
    core_research_call: &CoreResearchCall,
    execution_boundary_complete: bool,
    portfolio_decision: &StructuredPortfolioDecision,
) -> LocalText {
    match core_research_call {
        CoreResearchCall::LeanBuy => LocalText::new("state_lean_buy"),
        CoreResearchCall::BuyOnConfirmation => {
            let level = visible_confirmation_reference(portfolio_decision).unwrap_or_default();
            LocalText::new("state_buy_on_confirmation").with_str("confirmation", level)
        }
        CoreResearchCall::LeanSell => LocalText::new("state_lean_sell"),
        CoreResearchCall::SellOnBreak => {
            let level = visible_invalidation_reference(portfolio_decision, None).unwrap_or_default();
            LocalText::new("state_sell_on_break").with_str("invalidation", level)
        }
        CoreResearchCall::Neutral => {
            if execution_boundary_complete {
                LocalText::new("state_neutral_boundary_complete")
            } else {
                LocalText::new("state_neutral_boundary_incomplete")
            }
        }
    }
}

fn build_decision_action_line(
    action: &DecisionAction,
    portfolio_decision: &StructuredPortfolioDecision,
    execution_boundary_complete: bool,
) -> LocalText {
    match action {
        DecisionAction::BuyNow => LocalText::new("action_buy_now"),
        DecisionAction::ProbePosition => {
            let level = visible_confirmation_reference(portfolio_decision).unwrap_or_default();
            LocalText::new("action_probe_position").with_str("confirmation", level)
        }
        DecisionAction::WaitBreakout => {
            let level = visible_confirmation_reference(portfolio_decision).unwrap_or_default();
            LocalText::new("action_wait_breakout").with_str("confirmation", level)
        }
        DecisionAction::WaitRetest => LocalText::new("action_wait_retest"),
        DecisionAction::Reduce => LocalText::new("action_reduce"),
        DecisionAction::Exit => LocalText::new("action_exit"),
        DecisionAction::Hold => {
            if execution_boundary_complete {
                LocalText::new("action_hold_boundary_complete")
            } else {
                LocalText::new("action_hold_boundary_incomplete")
            }
        }
    }
}

fn build_decision_risk_line(portfolio_decision: &StructuredPortfolioDecision) -> LocalText {
    if let Some(invalidation_level) = visible_invalidation_reference(portfolio_decision, None) {
        return LocalText::new("risk_line_with_invalidation").with_str("invalidation", invalidation_level);
    }
    LocalText::new("risk_line_generic")
}

fn normalize_level_phrase(value: &str) -> String {
    let mut normalized = value.trim().trim_matches('。').trim().to_string();
    for prefix in [
        "价格有效突破并站稳",
        "价格有效处理并站稳",
        "价格有效站稳",
        "若失守",
        "失守",
        "有效跌破",
        "跌破",
    ] {
        if normalized.starts_with(prefix) {
            normalized = normalized[prefix.len()..].trim().to_string();
            break;
        }
    }
    normalized.trim_matches('。').trim().to_string()
}

fn normalize_trigger_phrase(value: &str) -> String {
    let normalized = value
        .trim()
        .trim_matches('。')
        .trim()
        .replace("价格有效突破并站稳 价格有效站稳", "价格有效站稳");
    if let Some(rest) = normalized.strip_prefix("价格有效突破并站稳") {
        let level = normalize_level_phrase(rest);
        return format!("触发升级需要满足：{}。", level);
    }
    if let Some(rest) = normalized.strip_prefix("若失守") {
        let level = normalize_level_phrase(rest);
        return format!("下调条件是：{}。", level);
    }
    normalized.trim_matches('。').trim().to_string()
}

fn normalize_reference_phrase(value: &str) -> String {
    value
        .trim()
        .trim_matches('。')
        .trim()
        .trim_end_matches("一带")
        .trim()
        .to_string()
}

fn is_publishable_summary_reference(value: &str) -> bool {
    let normalized = normalize_reference_phrase(value);
    if normalized.is_empty() {
        return false;
    }
    if normalized.chars().count() <= 1 {
        return false;
    }
    if normalized.contains("确认后")
        || normalized.contains("再评估")
        || normalized.contains("若补齐数据")
        || normalized.contains("升级为可执行")
        || normalized.contains("当前主张需要下修")
    {
        return false;
    }
    parse_first_numeric(&normalized).is_some()
        || normalized.contains("站稳")
        || normalized.contains("突破")
        || normalized.contains("跌破")
        || normalized.contains("失守")
        || normalized.contains("量价")
}

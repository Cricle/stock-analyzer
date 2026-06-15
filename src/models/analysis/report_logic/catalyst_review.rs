// Derive catalyst score card and review checklist from existing report data.
// These structures were previously always empty (Default::default()).
// This module populates them from trigger_checklist, news_insights,
// decision_view, portfolio_decision, and technical_indicators.

/// Derive a catalyst score card from existing news insights, trigger checklist,
/// and portfolio decision data.
fn derive_catalyst_score_card(
    news_insights: &[NewsInsight],
    portfolio_decision: &StructuredPortfolioDecision,
    decision_view: &DecisionView,
) -> CatalystScoreCard {
    let mut items = Vec::new();

    // Score news catalysts
    let has_bullish_news = news_insights
        .iter()
        .any(|n| n.impact_direction.as_str() == "bullish" || n.impact_direction.as_str() == "positive");
    let has_bearish_news = news_insights
        .iter()
        .any(|n| n.impact_direction.as_str() == "bearish" || n.impact_direction.as_str() == "negative" || n.impact_direction.as_str() == "caution");
    let has_strong_news = news_insights
        .iter()
        .any(|n| n.impact_strength.as_str() == "strong" || n.impact_strength.as_str() == "significant" || n.impact_strength.as_str() == "medium");

    items.push(CatalystScoreItem {
        question: "近期是否存在明确的利好催化剂？".into(),
        score: if has_bullish_news { 1 } else { 0 },
        evidence: if has_bullish_news {
            format!(
                "发现 {} 条利好新闻",
                news_insights
                    .iter()
                    .filter(|n| n.impact_direction.as_str() == "bullish"
                        || n.impact_direction.as_str() == "positive")
                    .count()
            ).into()
        } else {
            "未发现明确利好催化剂".into()
        },
    });

    items.push(CatalystScoreItem {
        question: "催化剂是否具备足够强度驱动价格重估？".into(),
        score: if has_strong_news { 1 } else { 0 },
        evidence: if has_strong_news {
            "存在强影响力催化剂".into()
        } else {
            "催化剂强度不足或尚需验证".into()
        },
    });

    // Score trigger checklist readiness
    let trigger_count = portfolio_decision.trigger_checklist.len();
    items.push(CatalystScoreItem {
        question: "升级触发条件是否已明确列出？".into(),
        score: if trigger_count >= 2 { 1 } else { 0 },
        evidence: format!("已列出 {} 条触发条件", trigger_count).into(),
    });

    // Score confirmation level
    let has_confirmation = !decision_view.confirmation_level.trim().is_empty();
    items.push(CatalystScoreItem {
        question: "是否存在明确的价格确认位？".into(),
        score: if has_confirmation { 1 } else { 0 },
        evidence: if has_confirmation {
            format!("确认位: {}", decision_view.confirmation_level).into()
        } else {
            "未设定明确确认位".into()
        },
    });

    // Score blocking gaps
    let blocking_gap_count = portfolio_decision
        .missing_evidence_ladder
        .blocking_gaps
        .len();
    items.push(CatalystScoreItem {
        question: "是否存在决策阻断缺口？".into(),
        score: if blocking_gap_count == 0 { 1 } else { 0 },
        evidence: if blocking_gap_count == 0 {
            "无决策阻断缺口".into()
        } else {
            format!("存在 {} 个决策阻断缺口", blocking_gap_count).into()
        },
    });

    // Score bearish news as negative catalyst
    items.push(CatalystScoreItem {
        question: "近期是否存在明确的利空催化剂？".into(),
        score: if has_bearish_news { 0 } else { 1 },
        evidence: if has_bearish_news {
            format!(
                "发现 {} 条利空新闻",
                news_insights
                    .iter()
                    .filter(|n| n.impact_direction.as_str() == "bearish"
                        || n.impact_direction.as_str() == "negative"
                        || n.impact_direction.as_str() == "caution")
                    .count()
            ).into()
        } else {
            "未发现明确利空催化剂".into()
        },
    });

    let total_score: i32 = items.iter().map(|item| item.score).sum();
    let max_score = items.len() as i32;

    let (event_name, interpretation, recommended_action) = if news_insights.is_empty() {
        (
            "待观察".into(),
            "暂无明确催化剂事件，需持续跟踪新闻和基本面变化。".into(),
            "保持观察，等待催化剂出现后再评估升级条件。".into(),
        )
    } else {
        let event_name = news_insights
            .first()
            .map(|n| n.title.chars().take(20).collect::<String>())
            .unwrap_or_else(|| "近期事件".to_string());
        let interpretation = match total_score {
            5..=6 => "积极 — 催化剂条件较为完备，可关注确认信号。",
            3..=4 => "中性 — 部分催化剂条件满足，仍需验证。",
            _ => "谨慎 — 催化剂条件不足或存在明显利空，建议等待更多证据。",
        };
        let recommended_action = match total_score {
            5..=6 => "关注价格是否触及确认位，满足条件后可考虑升级。",
            3..=4 => "继续跟踪催化剂发展，等待更多验证信号。",
            _ => "保持观望，等待催化剂条件改善。",
        };
        (event_name, interpretation.into(), recommended_action.into())
    };

    CatalystScoreCard {
        event_name,
        items,
        total_score,
        max_score,
        interpretation,
        recommended_action,
    }
}

/// Derive a review checklist from existing report data.
fn derive_review_checklist(
    decision_view: &DecisionView,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    price_context: &PriceContext,
    technical_indicators: &TechnicalIndicatorView,
    risk_controls: &[RiskControl],
) -> ReviewChecklist {
    let mut daily = Vec::new();
    let mut weekly = Vec::new();

    // --- Daily checklist ---

    // Price vs confirmation/invalidation
    if !decision_view.confirmation_level.trim().is_empty() {
        daily.push(ReviewItem {
            check: LocalText::new("review_price_near_confirmation")
                .with_str("level", &decision_view.confirmation_level)
                .with_f64("distance_pct", decision_view.distance_to_confirmation_pct),
            category: "price".to_string(),
            priority: "high".to_string(),
        });
    }

    if !decision_view.invalidation_level.trim().is_empty() {
        daily.push(ReviewItem {
            check: LocalText::new("review_price_near_invalidation")
                .with_str("level", &decision_view.invalidation_level)
                .with_f64("distance_pct", decision_view.distance_to_invalidation_pct),
            category: "price".to_string(),
            priority: "high".to_string(),
        });
    }

    // Entry/stop levels
    if !trader_plan.entry_price.trim().is_empty() {
        daily.push(ReviewItem {
            check: LocalText::new("review_entry_price_touched").with_str("price", &trader_plan.entry_price),
            category: "price".to_string(),
            priority: "medium".to_string(),
        });
    }

    // Use invalidation_level as the primary stop reference; fall back to stop_loss.
    // This prevents the review checklist from showing a different stop number
    // than the report's invalidation level.
    let effective_stop = if !portfolio_decision.invalidation_level.trim().is_empty() {
        portfolio_decision.invalidation_level.trim()
    } else {
        trader_plan.stop_loss.trim()
    };
    if !effective_stop.is_empty() {
        daily.push(ReviewItem {
            check: LocalText::new("review_stop_triggered").with_str("stop", effective_stop),
            category: "discipline".to_string(),
            priority: "high".to_string(),
        });
    }

    // Volume check
    if let Some(volume_change) = price_context.volume_change_pct {
        daily.push(ReviewItem {
            check: LocalText::new("review_volume_change").with_f64("change_pct", volume_change),
            category: "technical".to_string(),
            priority: "medium".to_string(),
        });
    }

    // Technical indicator warnings
    for conclusion in &technical_indicators.conclusions {
        if conclusion.severity == "warning" || conclusion.severity == "alert" {
            daily.push(ReviewItem {
                check: LocalText::new("review_technical_signal")
                    .with_str("signal", &conclusion.key)
                    .with_str("severity", &conclusion.severity),
                category: "technical".to_string(),
                priority: if conclusion.severity == "alert" {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
            });
        }
    }

    // Risk control monitoring
    for risk in risk_controls.iter().take(2) {
        if risk.probability_pct > 20.0 {
            daily.push(ReviewItem {
                check: LocalText::new("review_risk_item")
                    .with_str("risk_name", &risk.risk_name.key)
                    .with_f64("probability", risk.probability_pct)
                    .with_str("trigger", &risk.trigger.key),
                category: "discipline".to_string(),
                priority: "high".to_string(),
            });
        }
    }

    // --- Weekly checklist ---

    // Trigger checklist items
    for trigger in portfolio_decision.trigger_checklist.iter().take(3) {
        weekly.push(ReviewItem {
            check: LocalText::new("review_upgrade_trigger").with_str("trigger", trigger),
            category: "discipline".to_string(),
            priority: "high".to_string(),
        });
    }

    // Blocking gaps
    for gap in portfolio_decision
        .missing_evidence_ladder
        .blocking_gaps
        .iter()
        .take(2)
    {
        weekly.push(ReviewItem {
            check: LocalText::new("review_blocking_gap_cleared").with_str("gap", gap),
            category: "fundamental".to_string(),
            priority: "high".to_string(),
        });
    }

    // Manageable gaps
    for gap in portfolio_decision
        .missing_evidence_ladder
        .manageable_gaps
        .iter()
        .take(2)
    {
        weekly.push(ReviewItem {
            check: LocalText::new("review_manageable_gap_progress").with_str("gap", gap),
            category: "fundamental".to_string(),
            priority: "medium".to_string(),
        });
    }

    // Time stop deadline
    if !trader_plan.time_stop_deadline.trim().is_empty() {
        weekly.push(ReviewItem {
            check: LocalText::new("review_time_stop_deadline")
                .with_str("deadline", &trader_plan.time_stop_deadline)
                .with_str("reason", &trader_plan.time_stop_reason),
            category: "discipline".to_string(),
            priority: "high".to_string(),
        });
    }

    // Early probe conditions
    if decision_view.early_probe_allowed
        && !decision_view.early_probe_trigger.key.is_empty()
    {
        weekly.push(ReviewItem {
            check: LocalText::new("review_early_probe_trigger").with_str("trigger", &decision_view.early_probe_trigger.key),
            category: "discipline".to_string(),
            priority: "medium".to_string(),
        });
    }

    // Upgrade/downgrade conditions
    if !decision_view.next_upgrade_condition.key.is_empty() {
        weekly.push(ReviewItem {
            check: LocalText::new("review_upgrade_condition").with_str("condition", &decision_view.next_upgrade_condition.key),
            category: "discipline".to_string(),
            priority: "medium".to_string(),
        });
    }

    if !decision_view.next_downgrade_condition.key.is_empty() {
        weekly.push(ReviewItem {
            check: LocalText::new("review_downgrade_condition").with_str("condition", &decision_view.next_downgrade_condition.key),
            category: "discipline".to_string(),
            priority: "medium".to_string(),
        });
    }

    // Sort by priority and cap
    daily.sort_by_key(|a| priority_rank(&a.priority));
    daily.truncate(8);

    weekly.sort_by_key(|a| priority_rank(&a.priority));
    weekly.truncate(8);

    ReviewChecklist { daily, weekly }
}

fn priority_rank(p: &str) -> u8 {
    match p {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    }
}

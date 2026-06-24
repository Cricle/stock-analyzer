
fn build_scenario_paths(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    audience_mode: &str,
    blocker_present: bool,
    weak_history: bool,
) -> Vec<ActionScenarioPath> {
    let confirm = visible_confirmation_reference(portfolio_decision)
        .or_else(|| {
            let target = portfolio_decision.price_target.trim();
            (!target.is_empty()).then(|| target.to_string())
        })
        .unwrap_or_default();
    let entry = trader_plan.entry_price.trim();
    let stop = visible_invalidation_reference(portfolio_decision, Some(trader_plan))
        .unwrap_or_else(|| trader_plan.stop_loss.trim().to_string());
    let mut paths = vec![
        ActionScenarioPath {
            key: "breakout_continuation".to_string(),
            name: LocalText::new("path_name_breakout_continuation"),
            trigger: if confirm.is_empty() {
                LocalText::new("path_trigger_breakout_generic")
            } else {
                LocalText::new("path_trigger_breakout_confirm").with_str("confirm", &confirm)
            },
            action: match audience_mode {
                "holder" => LocalText::new("path_action_breakout_holder"),
                "buyer" => LocalText::new("path_action_breakout_buyer"),
                _ => LocalText::new("path_action_breakout_watcher"),
            },
            risk_boundary: if stop.is_empty() {
                LocalText::new("path_risk_breakout_generic")
            } else {
                LocalText::new("path_risk_breakout_with_stop").with_str("stop", &stop)
            },
            position_sizing: if blocker_present || weak_history {
                match audience_mode {
                    "holder" => LocalText::new("path_sizing_breakout_blocked_holder"),
                    _ => LocalText::new("path_sizing_breakout_blocked"),
                }
            } else {
                match audience_mode {
                    "holder" => LocalText::new("path_sizing_breakout_holder"),
                    "buyer" => LocalText::new("path_sizing_breakout_buyer"),
                    _ => LocalText::new("path_sizing_breakout_buyer"),
                }
            },
            stop_level: if stop.is_empty() {
                LocalText::default()
            } else {
                LocalText::new("path_stop_breakout").with_str("confirm", &confirm)
            },
            sizing_blocked: false,
        },
        ActionScenarioPath {
            key: "retest_confirmation".to_string(),
            name: LocalText::new("path_name_retest_confirmation"),
            trigger: if !confirm.is_empty() {
                LocalText::new("path_trigger_retest_confirm").with_str("confirm", &confirm)
            } else if !entry.is_empty() {
                LocalText::new("path_trigger_retest_entry").with_str("entry", entry)
            } else {
                LocalText::new("path_trigger_retest_generic")
            },
            action: match audience_mode {
                "holder" => LocalText::new("path_action_retest_holder"),
                "buyer" => LocalText::new("path_action_retest_buyer"),
                _ => LocalText::new("path_action_retest_watcher"),
            },
            risk_boundary: {
                let base_key = if stop.is_empty() {
                    "path_risk_retest_generic"
                } else {
                    "path_risk_retest_with_stop"
                };
                let mut rb = LocalText::new(base_key).with_str("stop", &stop);
                // Intermediate zone guidance
                let entry_f = entry.trim().parse::<f64>().ok();
                let stop_f = stop.trim().parse::<f64>().ok();
                if let (Some(ep), Some(sp)) = (entry_f, stop_f) {
                    let gap_pct = (ep - sp) / ep * 100.0;
                    if gap_pct >= 2.0 && sp > 0.0 {
                        rb = LocalText::new("path_risk_retest_intermediate_zone")
                            .with_str("entry", entry)
                            .with_str("stop", &stop)
                            .with_f64("midpoint", (ep + sp) / 2.0);
                    }
                }
                rb
            },
            position_sizing: if blocker_present || weak_history {
                match audience_mode {
                    "holder" => LocalText::new("path_sizing_retest_blocked_holder"),
                    _ => LocalText::new("path_sizing_retest_blocked"),
                }
            } else {
                match audience_mode {
                    "holder" => LocalText::new("path_sizing_retest_holder"),
                    "buyer" => LocalText::new("path_sizing_retest_buyer"),
                    _ => LocalText::new("path_sizing_retest_buyer"),
                }
            },
            stop_level: if !stop.is_empty() {
                LocalText::new("path_stop_retest_with_stop").with_str("stop", &stop)
            } else if !entry.is_empty() {
                LocalText::new("path_stop_retest_with_entry").with_str("entry", entry)
            } else {
                LocalText::new("path_stop_retest_generic")
            },
            sizing_blocked: false,
        },
        ActionScenarioPath {
            key: "failed_breakdown".to_string(),
            name: LocalText::new("path_name_failed_breakdown"),
            trigger: if stop.is_empty() {
                LocalText::new("path_trigger_breakdown_generic")
            } else {
                LocalText::new("path_trigger_breakdown_with_stop").with_str("stop", &stop)
            },
            action: match audience_mode {
                "holder" => {
                    let stop_f = stop.trim().parse::<f64>().ok();
                    let entry_f = entry.trim().parse::<f64>().ok();
                    let distance = match (entry_f, stop_f) {
                        (Some(e), Some(s)) if e > 0.0 && s < e => (e - s) / e * 100.0,
                        _ => 0.0,
                    };
                    if distance > 10.0 && !stop.is_empty() {
                        LocalText::new("path_action_breakdown_holder_layered").with_str("stop", &stop)
                    } else {
                        LocalText::new("path_action_breakdown_holder_simple")
                    }
                },
                "buyer" => LocalText::new("path_action_breakdown_buyer"),
                _ => LocalText::new("path_action_breakdown_watcher"),
            },
            risk_boundary: LocalText::new("path_risk_breakdown"),
            position_sizing: LocalText::new("path_sizing_breakdown"),
            stop_level: LocalText::new("path_stop_breakdown"),
            sizing_blocked: false,
        },
    ];
    paths.retain(|item| {
        !(item.trigger.key.is_empty() && item.action.key.is_empty() && item.risk_boundary.key.is_empty())
    });
    paths
}

fn collect_key_review_points(
    research_plan: &StructuredResearchPlan,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> Vec<LocalText> {
    let mut points = Vec::new();
    if !portfolio_decision.confirmation_level.trim().is_empty() {
        let level = visible_confirmation_reference(portfolio_decision)
            .unwrap_or_else(|| portfolio_decision.confirmation_level.trim().to_string());
        points.push(LocalText::new("review_observe_confirmation").with_str("level", &level));
    } else if !portfolio_decision.price_target.trim().is_empty() {
        points.push(LocalText::new("review_observe_target").with_str("target", portfolio_decision.price_target.trim()));
    }
    if let Some(invalidation_reference) =
        visible_invalidation_reference(portfolio_decision, Some(trader_plan))
    {
        points.push(LocalText::new("review_watch_invalidation").with_str("level", &invalidation_reference));
    }
    for item in portfolio_decision.trigger_checklist.iter().take(3) {
        points.push(LocalText::new("review_trigger_item").with_str("item", item));
    }
    for item in research_plan
        .missing_evidence_ladder
        .blocking_gaps
        .iter()
        .take(2)
    {
        points.push(LocalText::new("review_blocking_gap").with_str("gap", item));
    }
    points.dedup_by(|a, b| a.key == b.key && a.params == b.params);
    points
}

fn build_holder_actions(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    blocker_present: bool,
) -> Vec<LocalText> {
    let mut actions = Vec::new();
    actions.push(LocalText::new("action_holder_base"));
    if !trader_plan.stop_loss.trim().is_empty() {
        actions.push(LocalText::new("action_holder_stop").with_str("stop", trader_plan.stop_loss.trim()));
    }
    if !portfolio_decision.confirmation_level.trim().is_empty() {
        actions.push(LocalText::new("action_holder_confirm").with_str("confirmation", portfolio_decision.confirmation_level.trim()));
    } else if !portfolio_decision.price_target.trim().is_empty() {
        actions.push(LocalText::new("action_holder_target").with_str("target", portfolio_decision.price_target.trim()));
    }
    if blocker_present {
        actions.push(LocalText::new("action_holder_lock_profit"));
    }
    actions
}

fn build_buyer_actions(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    blocker_present: bool,
) -> Vec<LocalText> {
    let mut actions = Vec::new();
    if !portfolio_decision.confirmation_level.trim().is_empty() {
        actions.push(LocalText::new("action_buyer_confirm").with_str("confirmation", portfolio_decision.confirmation_level.trim()));
    } else if !portfolio_decision.price_target.trim().is_empty() {
        actions.push(LocalText::new("action_buyer_target").with_str("target", portfolio_decision.price_target.trim()));
    }
    if !trader_plan.entry_price.trim().is_empty() {
        let pullback_ref = visible_confirmation_reference(portfolio_decision)
            .unwrap_or_else(|| trader_plan.entry_price.trim().to_string());
        actions.push(LocalText::new("action_buyer_pullback").with_str("pullback_ref", &pullback_ref));
    }
    actions.push(LocalText::new("action_buyer_light_position"));
    if blocker_present {
        actions.push(LocalText::new("action_buyer_wait_validation"));
    }
    actions
}

fn build_watcher_actions(
    research_plan: &StructuredResearchPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    weak_history: bool,
) -> Vec<LocalText> {
    let mut actions = Vec::new();
    for item in portfolio_decision
        .missing_evidence_ladder
        .blocking_gaps
        .iter()
        .take(3)
    {
        actions.push(LocalText::new("action_watcher_validate").with_str("gap", item));
    }
    for item in research_plan.trigger_checklist.iter().take(2) {
        actions.push(LocalText::new("action_watcher_trigger").with_str("trigger", item));
    }
    if weak_history {
        actions.push(LocalText::new("action_watcher_weak_history"));
    }
    actions
}

pub fn render_action_guides_markdown(guides: &ReportActionGuides) -> String {
    [
        render_single_action_guide(&guides.holders),
        render_single_action_guide(&guides.buyers),
        render_single_action_guide(&guides.watchers),
    ]
    .join("\n\n")
}

fn render_single_action_guide(guide: &AudienceActionGuide) -> String {
    let execution_references = [
        (!guide.entry_reference.trim().is_empty()).then(|| {
            format!("- {}: {}", LocalText::new("label_entry_reference").key, guide.entry_reference.trim())
        }),
        (!guide.invalidation_reference.trim().is_empty()).then(|| {
            format!("- {}: {}", LocalText::new("label_invalidation_reference").key, guide.invalidation_reference.trim())
        }),
        (!guide.confirmation_reference.trim().is_empty()).then(|| {
            format!("- {}: {}", LocalText::new("label_confirmation_reference").key, guide.confirmation_reference.trim())
        }),
        (!guide.target_reference.trim().is_empty()).then(|| {
            format!("- {}: {}", LocalText::new("label_target_reference").key, guide.target_reference.trim())
        }),
        (!guide.time_horizon.trim().is_empty()).then(|| {
            format!("- {}: {}", LocalText::new("label_time_horizon").key, guide.time_horizon.trim())
        }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let none_label = LocalText::new("label_none").key;
    let execution_references = if execution_references.is_empty() {
        format!("- {none_label}\n")
    } else {
        execution_references.join("\n")
    };
    let actions = if guide.actions.is_empty() {
        format!("- {none_label}\n")
    } else {
        guide
            .actions
            .iter()
            .map(|item| format!("- {}", item.key))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let avoid = if guide.avoid.is_empty() {
        format!("- {none_label}\n")
    } else {
        guide
            .avoid
            .iter()
            .map(|item| format!("- {}", item.key))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let review = if guide.review_points.is_empty() {
        format!("- {none_label}\n")
    } else {
        guide
            .review_points
            .iter()
            .map(|item| format!("- {}", item.key))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let scenario_paths = if guide.scenario_paths.is_empty() {
        format!("- {none_label}\n")
    } else {
        guide
            .scenario_paths
            .iter()
            .map(|item| {
                let mut path_text = format!(
                    "- {}\n  {}: {}\n  {}: {}\n  {}: {}",
                    item.name.key,
                    LocalText::new("label_trigger").key, item.trigger.key,
                    LocalText::new("label_action").key, item.action.key,
                    LocalText::new("label_risk_boundary").key, item.risk_boundary.key
                );
                if !item.position_sizing.key.is_empty() {
                    path_text.push_str(&format!("\n  {}: {}", LocalText::new("label_position_sizing").key, item.position_sizing.key));
                }
                if !item.stop_level.key.is_empty() {
                    path_text.push_str(&format!("\n  {}: {}", LocalText::new("label_stop_level").key, item.stop_level.key));
                }
                path_text
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "### {}\n- {}: {}\n- {}: **{}**\n- {}: **{}**\n- {}: {}\n- {}: {}\n\n#### {}\n{}\n\n#### {}\n{}\n\n#### {}\n{}\n\n#### {}\n{}\n\n#### {}\n{}",
        guide.audience.key,
        LocalText::new("label_applicable_state").key, guide.user_state.key,
        LocalText::new("label_priority").key, guide.priority,
        LocalText::new("label_current_stance").key, guide.stance.key,
        LocalText::new("label_execution_principle").key, guide.principle.key,
        LocalText::new("label_summary").key, guide.summary.key,
        LocalText::new("label_execution_params").key,
        execution_references,
        LocalText::new("label_suggested_actions").key,
        actions,
        LocalText::new("label_scenario_paths").key,
        scenario_paths,
        LocalText::new("label_do_not").key,
        avoid,
        LocalText::new("label_next_review").key,
        review
    )
}

fn summarize_stage_state(stage_state: &ReportStageState) -> LocalText {
    let stage_keys = [
        (stage_state.overview, "stage_overview"),
        (stage_state.market, "stage_market"),
        (stage_state.fundamentals, "stage_fundamentals"),
        (stage_state.news, "stage_news"),
        (stage_state.sentiment, "stage_sentiment"),
        (stage_state.bull_research, "stage_bull_research"),
        (stage_state.bear_research, "stage_bear_research"),
        (stage_state.research_plan, "stage_research_plan"),
        (stage_state.trader_plan, "stage_trader_plan"),
        (stage_state.risk_debate, "stage_risk_debate"),
        (stage_state.portfolio_decision, "stage_portfolio_decision"),
        (stage_state.reflection, "stage_reflection"),
    ];

    let completed: Vec<serde_json::Value> = stage_keys
        .into_iter()
        .filter(|&(done, _key)| done).map(|(_done, key)| serde_json::Value::String(key.to_string()))
        .collect();

    if completed.is_empty() {
        LocalText::new("stage_state_none")
    } else {
        let mut params = serde_json::Map::new();
        params.insert("stages".to_string(), serde_json::Value::Array(completed));
        LocalText { key: "stage_state_summary".to_string(), params }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- build_scenario_paths ---

    #[test]
    fn scenario_paths_basic() {
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let paths = build_scenario_paths(&trader, &portfolio, "watcher", false, false);
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0].key, "breakout_continuation");
        assert_eq!(paths[1].key, "retest_confirmation");
        assert_eq!(paths[2].key, "failed_breakdown");
    }

    #[test]
    fn scenario_paths_with_levels() {
        let trader = StructuredTraderPlan::default();
        let mut portfolio = StructuredPortfolioDecision::default();
        portfolio.confirmation_level = "105.0".to_string();
        portfolio.price_target = "120.0".to_string();
        let paths = build_scenario_paths(&trader, &portfolio, "buyer", false, false);
        assert!(!paths[0].trigger.key.is_empty());
    }

    #[test]
    fn scenario_paths_holder_mode() {
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let paths = build_scenario_paths(&trader, &portfolio, "holder", false, false);
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn scenario_paths_with_blocker() {
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let paths = build_scenario_paths(&trader, &portfolio, "buyer", true, false);
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn scenario_paths_with_weak_history() {
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let paths = build_scenario_paths(&trader, &portfolio, "buyer", false, true);
        assert_eq!(paths.len(), 3);
    }

    // --- collect_key_review_points ---

    #[test]
    fn review_points_with_confirmation() {
        let research = StructuredResearchPlan::default();
        let trader = StructuredTraderPlan::default();
        let mut portfolio = StructuredPortfolioDecision::default();
        portfolio.confirmation_level = "105".to_string();
        let points = collect_key_review_points(&research, &trader, &portfolio);
        assert!(!points.is_empty());
    }

    #[test]
    fn review_points_with_target() {
        let research = StructuredResearchPlan::default();
        let trader = StructuredTraderPlan::default();
        let mut portfolio = StructuredPortfolioDecision::default();
        portfolio.price_target = "120".to_string();
        let points = collect_key_review_points(&research, &trader, &portfolio);
        assert!(!points.is_empty());
    }

    #[test]
    fn review_points_with_triggers() {
        let research = StructuredResearchPlan::default();
        let trader = StructuredTraderPlan::default();
        let mut portfolio = StructuredPortfolioDecision::default();
        portfolio.trigger_checklist = vec!["突破105".to_string(), "站稳110".to_string()];
        let points = collect_key_review_points(&research, &trader, &portfolio);
        assert!(points.len() >= 2);
    }

    #[test]
    fn review_points_with_blocking_gaps() {
        let mut research = StructuredResearchPlan::default();
        research.missing_evidence_ladder.blocking_gaps = vec!["缺少催化证据".to_string()];
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let points = collect_key_review_points(&research, &trader, &portfolio);
        assert!(points.iter().any(|p| p.key.contains("blocking_gap")));
    }

    // --- build_holder_actions ---

    #[test]
    fn holder_actions_basic() {
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let actions = build_holder_actions(&trader, &portfolio, false);
        assert!(!actions.is_empty());
    }

    #[test]
    fn holder_actions_with_stop() {
        let mut trader = StructuredTraderPlan::default();
        trader.stop_loss = "95".to_string();
        let portfolio = StructuredPortfolioDecision::default();
        let actions = build_holder_actions(&trader, &portfolio, false);
        assert!(actions.len() >= 2);
    }

    #[test]
    fn holder_actions_with_confirmation() {
        let trader = StructuredTraderPlan::default();
        let mut portfolio = StructuredPortfolioDecision::default();
        portfolio.confirmation_level = "105".to_string();
        let actions = build_holder_actions(&trader, &portfolio, false);
        assert!(actions.len() >= 2);
    }

    #[test]
    fn holder_actions_with_blocker() {
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let actions = build_holder_actions(&trader, &portfolio, true);
        assert!(actions.iter().any(|a| a.key.contains("lock_profit")));
    }

    // --- build_buyer_actions ---

    #[test]
    fn buyer_actions_basic() {
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let actions = build_buyer_actions(&trader, &portfolio, false);
        assert!(!actions.is_empty());
    }

    #[test]
    fn buyer_actions_with_entry() {
        let mut trader = StructuredTraderPlan::default();
        trader.entry_price = "105".to_string();
        let portfolio = StructuredPortfolioDecision::default();
        let actions = build_buyer_actions(&trader, &portfolio, false);
        assert!(actions.len() >= 2);
    }

    #[test]
    fn buyer_actions_with_blocker() {
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let actions = build_buyer_actions(&trader, &portfolio, true);
        assert!(actions.iter().any(|a| a.key.contains("wait_validation")));
    }

    // --- build_watcher_actions ---

    #[test]
    fn watcher_actions_basic() {
        let research = StructuredResearchPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let actions = build_watcher_actions(&research, &portfolio, false);
        assert!(actions.is_empty());
    }

    #[test]
    fn watcher_actions_with_gaps() {
        let research = StructuredResearchPlan::default();
        let mut portfolio = StructuredPortfolioDecision::default();
        portfolio.missing_evidence_ladder.blocking_gaps = vec!["需要更多数据".to_string()];
        let actions = build_watcher_actions(&research, &portfolio, false);
        assert!(!actions.is_empty());
    }

    #[test]
    fn watcher_actions_with_triggers() {
        let mut research = StructuredResearchPlan::default();
        research.trigger_checklist = vec!["等待突破".to_string()];
        let portfolio = StructuredPortfolioDecision::default();
        let actions = build_watcher_actions(&research, &portfolio, false);
        assert!(!actions.is_empty());
    }

    #[test]
    fn watcher_actions_weak_history() {
        let research = StructuredResearchPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let actions = build_watcher_actions(&research, &portfolio, true);
        assert!(actions.iter().any(|a| a.key.contains("weak_history")));
    }

    // --- render_action_guides_markdown ---

    #[test]
    fn render_guides_markdown() {
        let guides = ReportActionGuides::default();
        let md = render_action_guides_markdown(&guides);
        assert!(!md.is_empty());
    }

    // --- summarize_stage_state ---

    #[test]
    fn stage_state_none() {
        let state = ReportStageState::default();
        let result = summarize_stage_state(&state);
        assert_eq!(result.key, "stage_state_none");
    }

    #[test]
    fn stage_state_some_completed() {
        let mut state = ReportStageState::default();
        state.overview = true;
        state.market = true;
        let result = summarize_stage_state(&state);
        assert_eq!(result.key, "stage_state_summary");
    }

    #[test]
    fn stage_state_all_completed() {
        let state = ReportStageState {
            overview: true,
            market: true,
            fundamentals: true,
            news: true,
            sentiment: true,
            bull_research: true,
            bear_research: true,
            research_plan: true,
            trader_plan: true,
            risk_debate: true,
            portfolio_decision: true,
            reflection: true,
        };
        let result = summarize_stage_state(&state);
        assert_eq!(result.key, "stage_state_summary");
    }
}

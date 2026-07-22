
fn build_scenario_paths(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    audience_mode: &str,
    blocker_present: bool,
    weak_history: bool,
) -> Vec<ActionScenarioPath> {
    if portfolio_decision.rating.is_bearish() {
        return build_bearish_scenario_paths(
            trader_plan,
            portfolio_decision,
            audience_mode,
            blocker_present,
            weak_history,
        );
    }
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

fn build_bearish_scenario_paths(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    audience_mode: &str,
    blocker_present: bool,
    weak_history: bool,
) -> Vec<ActionScenarioPath> {
    let confirmation = visible_confirmation_reference(portfolio_decision).unwrap_or_default();
    let invalidation = visible_invalidation_reference(portfolio_decision, Some(trader_plan))
        .unwrap_or_else(|| trader_plan.stop_loss.trim().to_string());
    let target = visible_target_reference(portfolio_decision).unwrap_or_default();
    let sizing_blocked = blocker_present || weak_history;
    let sizing = if sizing_blocked {
        LocalText::new("path_sizing_bearish_blocked")
    } else {
        LocalText::new("path_sizing_bearish_conditional")
    };

    vec![
        ActionScenarioPath {
            key: "breakdown_continuation".to_string(),
            name: LocalText::new("path_name_bearish_breakdown"),
            trigger: LocalText::new("path_trigger_bearish_breakdown")
                .with_str("confirmation", &confirmation),
            action: LocalText::new(match audience_mode {
                "holder" => "path_action_bearish_breakdown_holder",
                "buyer" => "path_action_bearish_breakdown_buyer",
                _ => "path_action_bearish_breakdown_watcher",
            }),
            risk_boundary: LocalText::new("path_risk_bearish_invalidation")
                .with_str("invalidation", &invalidation),
            position_sizing: sizing.clone(),
            stop_level: LocalText::new("path_stop_bearish_invalidation")
                .with_str("invalidation", &invalidation),
            sizing_blocked,
        },
        ActionScenarioPath {
            key: "failed_breakdown_reclaim".to_string(),
            name: LocalText::new("path_name_bearish_failed_breakdown"),
            trigger: LocalText::new("path_trigger_bearish_reclaim")
                .with_str("confirmation", &confirmation),
            action: LocalText::new(match audience_mode {
                "holder" => "path_action_bearish_reclaim_holder",
                "buyer" => "path_action_bearish_reclaim_buyer",
                _ => "path_action_bearish_reclaim_watcher",
            }),
            risk_boundary: LocalText::new("path_risk_bearish_target_unconfirmed")
                .with_str("target", &target),
            position_sizing: LocalText::new("path_sizing_bearish_reclaim"),
            stop_level: LocalText::new("path_stop_bearish_confirmation")
                .with_str("confirmation", &confirmation),
            sizing_blocked: true,
        },
        ActionScenarioPath {
            key: "trend_repair".to_string(),
            name: LocalText::new("path_name_bearish_trend_repair"),
            trigger: LocalText::new("path_trigger_bearish_trend_repair")
                .with_str("invalidation", &invalidation),
            action: LocalText::new(match audience_mode {
                "holder" => "path_action_bearish_repair_holder",
                "buyer" => "path_action_bearish_repair_buyer",
                _ => "path_action_bearish_repair_watcher",
            }),
            risk_boundary: LocalText::new("path_risk_bearish_thesis_cancelled"),
            position_sizing: LocalText::new("path_sizing_bearish_reassess"),
            stop_level: LocalText::new("path_stop_bearish_reassess"),
            sizing_blocked: true,
        },
    ]
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
    is_bearish: bool,
) -> Vec<LocalText> {
    if is_bearish {
        let mut actions = vec![LocalText::new("action_holder_bearish_reduce")];
        if !portfolio_decision.confirmation_level.trim().is_empty() {
            actions.push(
                LocalText::new("action_holder_bearish_confirmation")
                    .with_str("confirmation", portfolio_decision.confirmation_level.trim()),
            );
        }
        if !trader_plan.stop_loss.trim().is_empty() {
            actions.push(
                LocalText::new("action_holder_bearish_invalidation")
                    .with_str("invalidation", trader_plan.stop_loss.trim()),
            );
        }
        if blocker_present {
            actions.push(LocalText::new("action_bearish_no_active_short"));
        }
        return actions;
    }
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
    is_bearish: bool,
) -> Vec<LocalText> {
    if is_bearish {
        let mut actions = vec![LocalText::new("action_buyer_bearish_avoid_long")];
        if !portfolio_decision.invalidation_level.trim().is_empty() {
            actions.push(
                LocalText::new("action_buyer_bearish_wait_repair")
                    .with_str("invalidation", portfolio_decision.invalidation_level.trim()),
            );
        }
        if blocker_present {
            actions.push(LocalText::new("action_bearish_no_active_short"));
        }
        return actions;
    }
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
    is_bearish: bool,
) -> Vec<LocalText> {
    let mut actions = Vec::new();
    if is_bearish {
        actions.push(LocalText::new("action_watcher_bearish_wait"));
    }
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

/// Compute Render_action_guides_markdown.
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

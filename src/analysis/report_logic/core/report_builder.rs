impl StructuredReport {
    pub(crate) fn from_result(
        result: &AnalysisResult,
        calibration_profile: &crate::scoring::CalibrationProfile,
    ) -> Self {
        let research_plan = result.structured_research_plan();
        let mut trader_plan = result.structured_trader_plan();
        let mut portfolio_decision = result.structured_portfolio_decision();
        // Sanitize price fields from LLM output: reject values unreasonably far
        // from current market price.  The LLM sometimes hallucinates prices
        // (e.g. entry_price 13.0 for a stock trading near 62.63).  Clear the
        // field so downstream consumers (action guides, scenario paths, decision
        // view) don't propagate the bogus number.
        {
            let cp = latest_market_close(result);
            if let Some(cur) = cp.filter(|c| *c > 0.0) {
                let max_gap = 0.5; // 50% threshold
                for (label, value) in [
                    ("entry_price", &trader_plan.entry_price),
                    ("stop_loss", &trader_plan.stop_loss),
                    ("confirmation_level", &trader_plan.confirmation_level),
                ] {
                    if let Some(price) = extract_first_price(value)
                        && (price / cur - 1.0).abs() > max_gap {
                            tracing::warn!(
                                stock = %result.symbol,
                                field = label,
                                hallucinated_price = price,
                                current_price = cur,
                                "LLM hallucinated price rejected (>50% gap from market)"
                            );
                        }
                }
                if let Some(ep) = extract_first_price(&trader_plan.entry_price)
                    && (ep / cur - 1.0).abs() > max_gap {
                        trader_plan.entry_price.clear();
                    }
                if let Some(sl) = extract_first_price(&trader_plan.stop_loss)
                    && (sl / cur - 1.0).abs() > max_gap {
                        trader_plan.stop_loss.clear();
                    }
                if let Some(cl) = extract_first_price(&trader_plan.confirmation_level)
                    && (cl / cur - 1.0).abs() > max_gap {
                        trader_plan.confirmation_level.clear();
                    }
                // Recover entry_price from stop_loss when the LLM hallucinated
                // or omitted it.  Entering near the invalidation/support level is
                // the most natural fallback.
                if trader_plan.entry_price.trim().is_empty()
                    && !trader_plan.stop_loss.trim().is_empty() {
                        trader_plan.entry_price = trader_plan.stop_loss.clone();
                    }
                // Guard against entry/stop inversion: entry must be > stop for
                // long setups.  When the LLM sets entry below stop, swap them.
                if let (Some(entry), Some(stop)) = (
                    extract_first_price(&trader_plan.entry_price),
                    extract_first_price(&trader_plan.stop_loss),
                )
                    && entry > 0.0 && stop > 0.0 && entry < stop {
                        std::mem::swap(
                            &mut trader_plan.entry_price,
                            &mut trader_plan.stop_loss,
                        );
                    }
            }
        }
        normalize_execution_references(result, &trader_plan, &mut portfolio_decision);
        // Also sanitize portfolio_decision price fields that came directly from
        // the LLM (not backfilled from trader_plan).  confirmation_level and
        // invalidation_level can contain hallucinated numbers.
        {
            let cp = latest_market_close(result);
            if let Some(cur) = cp.filter(|c| *c > 0.0) {
                let max_gap = 0.5;
                if let Some(cl) = extract_first_price(&portfolio_decision.confirmation_level)
                    && (cl / cur - 1.0).abs() > max_gap {
                        tracing::warn!(
                            stock = %result.symbol,
                            hallucinated_price = cl,
                            current_price = cur,
                            "LLM hallucinated confirmation_level rejected"
                        );
                        portfolio_decision.confirmation_level.clear();
                    }
                if let Some(il) = extract_first_price(&portfolio_decision.invalidation_level)
                    && (il / cur - 1.0).abs() > max_gap {
                        tracing::warn!(
                            stock = %result.symbol,
                            hallucinated_price = il,
                            current_price = cur,
                            "LLM hallucinated invalidation_level rejected"
                        );
                        portfolio_decision.invalidation_level.clear();
                    }
            }
        }
        let reflection = StructuredReflection::from_text(&result.graph.reflection.reflection);
        let risk_assessment = StructuredRiskAssessment::from_text(&result.derived_risk_assessment());
        let confidence_assessment = crate::scoring::evaluate_confidence_score(
            result,
            &crate::config::SaConfig::load().score_config().caps,
            false, // consistency_flag set after technical_indicators are derived
            "",
        );
        let direction_assessment = crate::scoring::evaluate_direction_score(result);
        let action_assessment = crate::scoring::evaluate_action_score(
            result,
            &trader_plan,
            &portfolio_decision,
            direction_assessment.final_score,
            confidence_assessment.final_score,
        );
        let action_breakdown = action_assessment.breakdown.clone();
        let structural_execution_boundary =
            crate::scoring::has_execution_boundary(&trader_plan, &portfolio_decision);
        let mut diagnostics = derive_report_diagnostics(result);
        enrich_diagnostic_linkage(
            &mut diagnostics,
            &research_plan,
            &trader_plan,
            &portfolio_decision,
        );
        let execution_blocking_gaps = collect_execution_blocking_gaps(
            &research_plan,
            &trader_plan,
            &portfolio_decision,
            &diagnostics,
        );
        for gap in &execution_blocking_gaps {
            if !portfolio_decision
                .missing_evidence_ladder
                .blocking_gaps
                .iter()
                .any(|existing| existing == gap)
            {
                portfolio_decision
                    .missing_evidence_ladder
                    .blocking_gaps
                    .push(gap.clone());
            }
        }
        let execution_boundary_complete =
            structural_execution_boundary && execution_blocking_gaps.is_empty();
        let missing_execution_fields =
            collect_missing_execution_fields(&trader_plan, &portfolio_decision);
        let research_reliability = derive_research_reliability(
            &confidence_assessment.breakdown,
            &confidence_assessment.caps,
            &result.artifacts.memory_context,
            execution_boundary_complete,
            &diagnostics,
        );
        let raw_llm_recommendation = result.derived_recommendation();
        let setup_direction_alignment = crate::scoring::score_setup_direction_alignment(result);
        let has_confirmation_boundary = !portfolio_decision.confirmation_level.trim().is_empty()
            && !trader_plan.entry_price.trim().is_empty()
            && !trader_plan.stop_loss.trim().is_empty();
        let current_confirmation_is_strong = has_confirmation_boundary
            && (action_assessment.breakdown.execution_levels.score >= 20
                || action_assessment.breakdown.reward_to_risk.score >= 9);
        let memory_direction_misaligned = setup_direction_alignment.score <= 4
            && result.artifacts.memory_context.setup_resolved_match_count >= 2
            && result.artifacts.memory_context.setup_neutral_match_count
                < result.artifacts.memory_context.setup_resolved_match_count;
        let positive_setup_support = result.artifacts.memory_context.setup_resolved_match_count >= 2
            && result.artifacts.memory_context.setup_match_hit_rate >= 0.6
            && result.artifacts.memory_context.setup_match_avg_alpha_return > 0.0
            && !memory_direction_misaligned;
        let direction_threshold_penalty = if memory_direction_misaligned {
            8
        } else if positive_setup_support && setup_direction_alignment.score >= 8 {
            -4
        } else if current_confirmation_is_strong
            && result.artifacts.memory_context.setup_neutral_match_count
                == result.artifacts.memory_context.setup_resolved_match_count
        {
            0
        } else if setup_direction_alignment.score <= 6
            && result.artifacts.memory_context.setup_resolved_match_count >= 2
        {
            4
        } else {
            0
        };
        let memory_threshold_tightened = result
            .artifacts
            .memory_context
            .used_setup_filtered_retrieval
            && (result.artifacts.memory_context.setup_resolved_match_count < 2
                || result.artifacts.memory_context.setup_match_hit_rate < 0.5
                || result.artifacts.memory_context.setup_match_avg_alpha_return <= 0.0
                || memory_direction_misaligned)
            && !current_confirmation_is_strong;
        let effective_confidence_score = if memory_threshold_tightened {
            confidence_assessment
                .final_score
                .min(calibration_profile.min_confidence_score + 10)
        } else if positive_setup_support {
            (confidence_assessment.final_score + 6).min(100)
        } else {
            confidence_assessment.final_score
        };
        let effective_action_score = if memory_threshold_tightened {
            action_assessment
                .final_score
                .min(calibration_profile.min_action_score + 8)
        } else if positive_setup_support {
            (action_assessment.final_score + 8).min(100)
        } else {
            action_assessment.final_score
        };
        let setup_match_quality = confidence_assessment
            .breakdown
            .historical_transferability
            .clone();
        let research_confidence_score = confidence_assessment
            .breakdown
            .total_before_caps
            .clamp(0, 100);
        let setup_match_explanation = derive_setup_match_explanation(
            &result.artifacts.memory_context,
            calibration_profile.sample_count,
        );
        let references = derive_report_references(
            result,
            &confidence_assessment.breakdown,
            &result.artifacts.memory_context,
        );
        let mut market_chart = result.artifacts.market_chart.clone();
        // Compute a rough reward/risk hint from execution levels for calibration.
        let reward_risk_hint = compute_reward_risk_hint(&trader_plan, &portfolio_decision);
        let calibration = crate::scoring::calibrate_recommendation_with_profile(
            &raw_llm_recommendation,
            direction_assessment.final_score,
            effective_confidence_score,
            effective_action_score,
            execution_boundary_complete,
            calibration_profile,
            direction_threshold_penalty,
            reward_risk_hint,
        );
        let calibration_rationale = {
            
            calibration.rationale.clone()
                .with_bool("memory_threshold_tightened", memory_threshold_tightened)
                .with_bool("memory_direction_misaligned", memory_direction_misaligned)
                .with_bool("positive_setup_support", positive_setup_support)
                .with_i32("effective_confidence", effective_confidence_score)
                .with_i32("effective_action", effective_action_score)
        };
        let threshold_tightened = crate::scoring::history_requires_caution(calibration_profile);
        let mut action_guides = derive_action_guides(
            result,
            &research_plan,
            &trader_plan,
            &portfolio_decision,
            &confidence_assessment.profile,
            &confidence_assessment.caps,
        );
        trader_plan.raw_action = trader_plan.action.as_str().to_string();
        trader_plan.calibrated_action = calibration.final_action.clone();
        trader_plan.action = LocalText::new(calibration.final_action.clone());
        portfolio_decision.raw_rating = portfolio_decision.rating.to_string();
        portfolio_decision.calibrated_rating = calibration.final_rating.clone();
        portfolio_decision.rating = Rating::parse(&calibration.final_rating);
        portfolio_decision.confidence = LocalText::new(effective_confidence_score.to_string());
        if portfolio_decision.risk_assessment.trim().is_empty() {
            portfolio_decision.risk_assessment = LocalText::new(result.derived_risk_assessment());
        }
        if portfolio_decision.invalidation_level.trim().is_empty() {
            portfolio_decision.invalidation_level = trader_plan.stop_loss.trim().to_string();
        }
        if portfolio_decision.target_reference.trim().is_empty() {
            portfolio_decision.target_reference = visible_target_reference(&portfolio_decision)
                .unwrap_or_default();
        }
        if portfolio_decision.target_type.trim().is_empty() {
            portfolio_decision.target_type =
                infer_target_type(&portfolio_decision, execution_boundary_complete).to_string();
        }
        if portfolio_decision.target_condition.trim().is_empty() {
            portfolio_decision.target_condition =
                infer_target_condition(&portfolio_decision, execution_boundary_complete);
        }
        // Override trader_plan.position_sizing when blocking gaps exist.
        // Without this, the "执行计划" section renders the LLM-generated sizing
        // (e.g. 2%) while the decision panel and action guides correctly show 0%.
        let has_blockers = !portfolio_decision.missing_evidence_ladder.blocking_gaps.is_empty()
            || !trader_plan.blocking_gaps.is_empty();
        if has_blockers {
            trader_plan.position_sizing = "0%——关键证据尚未补齐，不新增方向性暴露".to_string();
        }
        if Rating::parse(&portfolio_decision.raw_rating) != portfolio_decision.rating
        {
            portfolio_decision.investment_thesis = portfolio_decision
                .authoritative_investment_thesis(&trader_plan, confidence_assessment.final_score).into();
            portfolio_decision.rationale = portfolio_decision.authoritative_rationale(
                &trader_plan,
                confidence_assessment.final_score,
                &calibration.rationale.key,
            ).into();
        }
        let consensus = analyst_consensus(&result.graph.analysts);
        let core_research_call = derive_core_research_call(
            &research_plan,
            &raw_llm_recommendation,
            direction_assessment.final_score,
            research_confidence_score,
            &research_reliability,
            &portfolio_decision,
            consensus,
        );
        let forced_hold = !execution_boundary_complete
            && !matches!(
                core_research_call,
                CoreResearchCall::Neutral | CoreResearchCall::BuyOnConfirmation | CoreResearchCall::SellOnBreak
            );
        let current_price = latest_market_close(result);
        let decision_view = build_decision_view(
            &trader_plan,
            &portfolio_decision,
            &action_guides,
            effective_confidence_score,
            execution_boundary_complete,
            forced_hold,
            &core_research_call,
            current_price,
        );
        enrich_market_chart(&mut market_chart, &references, &decision_view);
        let price_context = derive_price_context(&market_chart, current_price);
        let technical_indicators = derive_technical_indicators(&market_chart);

        // Run validation checks on LLM output
        let validation_result = {
            let recommendation = result.derived_recommendation();
            let rsi = technical_indicators.categories
                .iter()
                .flat_map(|c| &c.indicators)
                .find(|t| t.key == "RSI")
                .and_then(|t| t.value)
                .unwrap_or(50.0);
            let macd_signal = technical_indicators.categories
                .iter()
                .flat_map(|c| &c.indicators)
                .find(|t| t.key == "MACD")
                .map(|t| t.signal_code.as_str())
                .unwrap_or("neutral");
            crate::analysis::validation::check_consistency(
                &recommendation, rsi, macd_signal,
            )
        };
        if validation_result.consistency_flag {
            tracing::warn!(
                stock = %result.symbol,
                reason = %validation_result.consistency_reason,
                "Recommendation contradicts technical indicators"
            );
        }

        let probability_view = derive_probability_view(
            &decision_view,
            direction_assessment.final_score,
            effective_confidence_score,
            &price_context,
            &result.artifacts.memory_context,
            &technical_indicators,
        );
        let profit_risk = derive_profit_risk(&decision_view, &price_context, &probability_view);
        let ic_navigator = derive_ic_navigator(&decision_view, &probability_view);
        let ic_discipline = derive_ic_discipline(
            &decision_view,
            &market_chart,
            &technical_indicators,
            &price_context,
            &probability_view,
            &profit_risk,
        );
        // When IC discipline says "no_attack", strip concrete buy sizing from
        // scenario paths to avoid contradicting the "禁止进攻" stance.
        if ic_discipline.state.as_str() == "no_attack" {
            sanitize_scenario_paths_for_no_attack(&mut action_guides);
        }
        // Upgrade forced_hold when IC discipline forbids directional action,
        // even if core_research_call is already Neutral.  This ensures the
        // execution_readiness correctly reflects the blocking state.
        let forced_hold = forced_hold || ic_discipline.state.as_str() == "no_attack";
        let evidence_cards = derive_evidence_cards(&references);
        let news_insights =
            derive_news_insights(&references, &decision_view, &price_context, &diagnostics, &result.analysis_date);
        let risk_controls = derive_risk_controls(
            &decision_view,
            &portfolio_decision,
            &research_reliability,
            &price_context,
            &probability_view,
        );
        {
            let llm_summary = portfolio_decision.executive_summary.clone();
            let template_summary = portfolio_decision.authoritative_summary(
                &trader_plan,
                effective_confidence_score,
                &core_research_call,
                &decision_view,
            );
            if llm_summary.key.len() > 20
                && !llm_summary.key.contains("Model did not return")
                && !llm_summary.key.contains("模型未返回")
            {
                tracing::info!(
                    task_id = %result.task_id,
                    symbol = %result.symbol,
                    summary_len = llm_summary.key.len(),
                    "using LLM-generated executive summary"
                );
            } else {
                portfolio_decision.executive_summary = LocalText::new(template_summary);
                tracing::info!(
                    task_id = %result.task_id,
                    symbol = %result.symbol,
                    "falling back to template executive summary"
                );
            }
        }
        append_scenario_gap_narrative(
            &mut portfolio_decision.executive_summary,
            &diagnostics,
            "当前不能升级结论的直接原因是",
        );
        append_scenario_gap_narrative(
            &mut portfolio_decision.risk_assessment,
            &diagnostics,
            "场景级阻断项",
        );
        // Append code-computed reward-risk ratio to executive_summary to prevent
        // the LLM's own conflicting ratio (e.g. 0.13) from appearing alongside
        // the authoritative computed value (e.g. 4.98).
        if let Some(rr) = profit_risk.reward_risk_ratio {
            let rr_label = crate::analysis::rr_label(rr);
            if let Some(crr) = profit_risk.current_position_reward_risk_ratio
                && (crr - rr).abs() > 0.01
            {
                let crr_label = crate::analysis::rr_label(crr);
                portfolio_decision.executive_summary.key.push_str(&format!(
                    " 系统计算盈亏比（当前→确认位）: {:.2}（{}），（当前→目标位）: {:.2}（{}），以代码计算值为准。",
                    crr, crr_label, rr, rr_label
                ));
            } else {
                portfolio_decision.executive_summary.key.push_str(&format!(
                    " 系统计算盈亏比: {:.2}（{}），以代码计算值为准。",
                    rr, rr_label
                ));
            }
        }
        let mispricing_claim = derive_mispricing_claim(
            &raw_llm_recommendation,
            &portfolio_decision,
            &research_reliability,
        );
        let why_now = derive_why_now(&decision_view, &portfolio_decision);
        let required_confirmation = derive_required_confirmation(&decision_view, &portfolio_decision);
        let max_initial_risk_budget = derive_max_initial_risk_budget(
            &decision_view,
            &confidence_assessment.caps,
            memory_threshold_tightened,
        );
        let appendix_reliability_summary = format!(
            "{} {}",
            derive_reliability_appendix_summary(&research_reliability, &result.artifacts.memory_context),
            calibration.decision_narrative.key
        )
        .trim()
        .to_string();
        let overview_section = build_overview_section(OverviewSectionParams {
            result,
            portfolio_decision: &portfolio_decision,
            recommendation: calibration.final_rating.as_str(),
            confidence_score: effective_confidence_score,
            research_confidence_score,
            research_reliability: &research_reliability,
            core_research_call: &core_research_call,
            decision_view: &decision_view,
            decision_narrative: &calibration.decision_narrative.key,
            mispricing_claim: &mispricing_claim,
            why_now: &why_now,
            required_confirmation: &required_confirmation,
            max_initial_risk_budget: &max_initial_risk_budget,
        });
        let mut sections = [
            (
                "market",
                "市场技术",
                result.agent_state.market_report.as_str(),
            ),
            (
                "fundamentals",
                "基本面",
                result.agent_state.fundamentals_report.as_str(),
            ),
            ("news", "新闻事件", result.agent_state.news_report.as_str()),
            (
                "sentiment",
                "资金情绪",
                result.agent_state.sentiment_report.as_str(),
            ),
            (
                "bull_case",
                "多头研究",
                result.graph.investment_debate.bull_history.as_str(),
            ),
            (
                "bear_case",
                "空头研究",
                result.graph.investment_debate.bear_history.as_str(),
            ),
            (
                "research_plan",
                "投资计划",
                result.agent_state.investment_plan.as_str(),
            ),
            (
                "trader_plan",
                "交易计划",
                result.agent_state.trader_investment_plan.as_str(),
            ),
            (
                "risk_debate",
                "风险辩论",
                result.agent_state.risk_debate_state.history.as_str(),
            ),
            (
                "portfolio_decision",
                "综合结论",
                result.agent_state.final_trade_decision.as_str(),
            ),
            (
                "reflection",
                "复盘反思",
                result.graph.reflection.reflection.as_str(),
            ),
        ]
        .into_iter()
        .filter_map(|(key, title, content)| {
            let trimmed = content.trim();
            (!trimmed.is_empty()).then(|| ReportSection {
                key: key.to_string(),
                title: title.to_string(),
                content: trimmed.to_string(),
            })
        })
        .collect::<Vec<_>>();
        if let Some(overview_section) = overview_section {
            sections.insert(0, overview_section);
        }
        sections.insert(
            1,
            ReportSection {
                key: "audience_guides".to_string(),
                title: "分场景行动建议".to_string(),
                content: render_action_guides_markdown(&action_guides),
            },
        );

        let catalyst_score_card = derive_catalyst_score_card(&news_insights, &portfolio_decision, &decision_view);
        let review_checklist = derive_review_checklist(&decision_view, &trader_plan, &portfolio_decision, &price_context, &technical_indicators, &risk_controls);
        Self {
            report_flavor: ReportFlavor::Execution,
            title: format!("{} / {}", result.symbol, result.stock_name).into(),
            summary: portfolio_decision.executive_summary.clone(),
            recommendation: calibration.final_rating.into(),
            raw_llm_recommendation,
            recommendation_calibration_reason: calibration_rationale.key,
            confidence: effective_confidence_score.to_string().into(),
            raw_llm_confidence: result.derived_confidence(),
            confidence_score: effective_confidence_score,
            confidence_breakdown: confidence_assessment.breakdown,
            confidence_profile: confidence_assessment.profile,
            confidence_caps: confidence_assessment.caps,
            research_reliability,
            research_confidence_score,
            direction_score: direction_assessment.final_score,
            direction_breakdown: direction_assessment.breakdown,
            action_score: action_assessment.final_score,
            action_breakdown: action_breakdown.clone(),
            execution_readiness: ExecutionReadiness {
                execution_boundary_complete,
                missing_execution_fields: missing_execution_fields.clone(),
                blocking_gaps: execution_blocking_gaps.clone(),
                forced_hold,
                forced_hold_reason: if forced_hold {
                    if missing_execution_fields.is_empty() && !execution_blocking_gaps.is_empty() {
                        LocalText::new("forced_hold_blocking_gaps")
                            .with_str("gaps", execution_blocking_gaps.join("；"))
                    } else {
                        LocalText::new("forced_hold_incomplete_boundary")
                    }
                } else {
                    LocalText::default()
                },
            },
            trade_setup_quality: derive_trade_setup_quality(
                &trader_plan,
                &portfolio_decision,
                &action_breakdown,
                execution_boundary_complete,
                &execution_blocking_gaps,
            ),
            calibration_summary: CalibrationSummary {
                threshold_tightened,
                memory_threshold_tightened,
                min_confidence_score: calibration_profile.min_confidence_score,
                min_action_score: calibration_profile.min_action_score,
                direction_floor_abs: if threshold_tightened {
                    (calibration_profile.direction_floor_abs + 5)
                        .min(calibration_profile.strong_direction_abs)
                } else {
                    calibration_profile.direction_floor_abs
                }
                .saturating_add(direction_threshold_penalty)
                .min(85),
                strong_direction_abs: if threshold_tightened {
                    (calibration_profile.strong_direction_abs + 5).min(85)
                } else {
                    calibration_profile.strong_direction_abs
                }
                .saturating_add(direction_threshold_penalty)
                .min(90),
                direction_threshold_penalty,
                historical: HistoricalCalibrationStats {
                    sample_count: calibration_profile.sample_count,
                    hit_rate: calibration_profile.min_hit_rate,
                    avg_alpha_return: calibration_profile.min_avg_alpha_return,
                },
                setup_calibration_sample_count: result
                    .artifacts
                    .memory_context
                    .setup_calibration_sample_count,
                setup_match_count: result.artifacts.memory_context.setup_match_count,
                setup_pending_match_count: result
                    .artifacts
                    .memory_context
                    .setup_pending_match_count,
                setup_match_explanation,
                setup_match_quality,
                setup_direction_alignment,
                calibration_bias: derive_calibration_bias(
                    &result.artifacts.memory_context,
                    memory_threshold_tightened,
                    memory_direction_misaligned,
                    positive_setup_support,
                ),
                decision_narrative: calibration.decision_narrative.clone(),
            },
            diagnostics,
            references,
            market_chart,
            user_context: result.artifacts.user_context.clone(),
            price_context,
            probability_view,
            profit_risk,
            ic_navigator,
            ic_discipline,
            technical_indicators,
            evidence_cards,
            news_insights,
            risk_controls,
            action_guides,
            decision_view,
            core_research_call: core_research_call.to_string().into(),
            mispricing_claim,
            why_now,
            required_confirmation,
            max_initial_risk_budget,
            appendix_reliability_summary,
            risk_assessment: risk_assessment.raw_text.clone().into(),
            rationale: result.derived_rationale().into(),
            research_plan,
            trader_plan,
            portfolio_decision,
            reflection,
            catalyst_score_card,
            review_checklist,
            stage_state: result.report_stage(),
            sections,
        }
    }
}

/// When IC discipline is "no_attack" (poor reward-risk or overheated RSI),
/// clear position sizing in buyer/watcher scenario paths and set the
/// structured `sizing_blocked` flag.  The frontend renders this as
/// observation-only without inventing a sizing number.
fn sanitize_scenario_paths_for_no_attack(action_guides: &mut ReportActionGuides) {
    for guide in [&mut action_guides.buyers, &mut action_guides.watchers] {
        for path in &mut guide.scenario_paths {
            path.position_sizing = LocalText::default();
            path.sizing_blocked = true;
        }
    }
}

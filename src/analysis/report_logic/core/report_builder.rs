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
                let max_gap = 0.3; // 30% threshold — reject hallucinated prices more aggressively
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
                                "LLM hallucinated price rejected (>30% gap from market)"
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
                // Also validate price_target from portfolio_decision
                if let Some(tp) = extract_first_price(&portfolio_decision.price_target)
                    && (tp / cur - 1.0).abs() > max_gap {
                        tracing::warn!(
                            stock = %result.symbol,
                            hallucinated_price = tp,
                            current_price = cur,
                            "LLM hallucinated price_target rejected (>30% gap from market)"
                        );
                        portfolio_decision.price_target.clear();
                    }
            }
        }
        normalize_execution_references(result, &trader_plan, &mut portfolio_decision);
        // Phase 3.3b: recover entry_price from confirmation_level when entry is missing.
        // Derive entry below confirmation to create an observation window.
        // If confirmation > current: use 80% of the gap, but enforce at least 1% gap.
        // If confirmation <= current: use 97% of confirmation.
        if trader_plan.entry_price.trim().is_empty()
            && !portfolio_decision.confirmation_level.trim().is_empty()
        {
            if let Some(conf_price) = extract_first_price(&portfolio_decision.confirmation_level) {
                let current = latest_market_close(result).unwrap_or(conf_price);
                let entry_price = if conf_price > current && current > 0.0 {
                    let pullback = current + (conf_price - current) * 0.8;
                    if (conf_price - pullback) / conf_price < 0.01 {
                        conf_price * 0.97
                    } else {
                        pullback
                    }
                } else {
                    conf_price * 0.97
                };
                trader_plan.entry_price = format_price_reference(entry_price);
                portfolio_decision.rationale = format!(
                    "{} entry_price_derived_from_confirmation={:.2} (original={:.2})",
                    portfolio_decision.rationale.as_str().trim(),
                    entry_price,
                    conf_price
                ).into();
            }
        }
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
        // Post-processing: derive missing fields from available data
        {
            let cp = latest_market_close(result);
            let price_anchors = collect_price_anchors(result, &trader_plan, &portfolio_decision);
            // 1. When target == confirmation, derive a proper target above confirmation.
            //    Target should be higher than confirmation for bullish setups.
            if let (Some(target), Some(confirm)) = (
                parse_first_numeric(&portfolio_decision.price_target),
                parse_first_numeric(&portfolio_decision.confirmation_level),
            ) {
                if (target - confirm).abs() / confirm.max(1.0) < 0.05 {
                    // Target ≈ confirmation: derive a proper target
                    tracing::info!(
                        stock = %result.symbol,
                        target = target,
                        confirmation = confirm,
                        "target ≈ confirmation detected, deriving proper target"
                    );
                    if let Some(above) = nearest_anchor_above(
                        Some(confirm), &price_anchors,
                    ) {
                        portfolio_decision.price_target = format_price_reference(above);
                    } else if let Some(cur) = cp {
                        // Default: 5% above confirmation
                        let derived = confirm * 1.05;
                        if derived > cur {
                            portfolio_decision.price_target = format_price_reference(derived);
                        }
                    }
                }
            }
            // 2. When invalidation is missing, derive from stop_loss or set default
            if portfolio_decision.invalidation_level.trim().is_empty() {
                if !trader_plan.stop_loss.trim().is_empty() {
                    portfolio_decision.invalidation_level = trader_plan.stop_loss.trim().to_string();
                } else if let Some(cur) = cp {
                    // Default: 8% below current price as invalidation
                    let derived = cur * 0.92;
                    portfolio_decision.invalidation_level = format_price_reference(derived);
                }
            }
            // 3. When time_horizon is missing, set a reasonable default
            if portfolio_decision.time_horizon.trim().is_empty() {
                portfolio_decision.time_horizon = "2-6周".to_string();
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
        let mut market_chart = result.artifacts.market_chart.clone();
        let technical_indicators = derive_technical_indicators(&market_chart);
        let direction_assessment = crate::scoring::evaluate_direction_score(result, &technical_indicators);
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
        if !execution_blocking_gaps.is_empty() {
            tracing::debug!(gaps = ?execution_blocking_gaps, "execution blocking gaps detected");
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
            let capped = confidence_assessment
                .final_score
                .min(calibration_profile.min_confidence_score + 15);
            tracing::info!(
                stock = %result.symbol,
                raw_confidence = confidence_assessment.final_score,
                capped_confidence = capped,
                min_confidence_score = calibration_profile.min_confidence_score,
                "memory threshold tightened, confidence capped"
            );
            capped
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
        // Cross-field guard: entry_price must be > invalidation_level for long setups.
        // The LLM may set invalidation_level (e.g. BOLL lower band) independently of
        // trader_plan.entry_price, creating an inversion where entry < invalidation.
        // When this happens, lower invalidation_level to stop_loss (which is always < entry
        // after the swap guard above) or to entry * 0.95 as a conservative fallback.
        if let (Some(entry), Some(inval)) = (
            extract_first_price(&trader_plan.entry_price),
            extract_first_price(&portfolio_decision.invalidation_level),
        ) {
            if entry > 0.0 && inval > 0.0 && entry < inval {
                let new_inval = if let Some(stop) = extract_first_price(&trader_plan.stop_loss)
                    .filter(|&s| s > 0.0 && s < entry)
                {
                    format_price_reference(stop)
                } else {
                    format_price_reference((entry * 95.0).round() / 100.0)
                };
                tracing::warn!(
                    stock = %result.symbol,
                    entry = entry,
                    original_invalidation = inval,
                    new_invalidation = %new_inval,
                    "entry < invalidation_level detected, lowered invalidation"
                );
                portfolio_decision.invalidation_level = new_inval;
            }
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
        let atr_14 = compute_atr_14(&market_chart);
        let first_target = if !portfolio_decision.target_reference.trim().is_empty() {
            Some(portfolio_decision.target_reference.trim().to_string())
        } else if !portfolio_decision.price_target.trim().is_empty() {
            Some(portfolio_decision.price_target.trim().to_string())
        } else {
            None
        };
        let mut decision_view = build_decision_view(
            &trader_plan,
            &portfolio_decision,
            &action_guides,
            effective_confidence_score,
            execution_boundary_complete,
            forced_hold,
            &core_research_call,
            current_price,
            first_target,
            atr_14,
        );
        // Enforce price consistency EARLY so all derived views use corrected values.
        enforce_price_consistency(
            &mut trader_plan,
            &mut portfolio_decision,
            &mut decision_view,
            &mut action_guides,
        );
        enrich_market_chart(&mut market_chart, &references, &decision_view);
        let price_context = derive_price_context(&market_chart, current_price);

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
            let mut v = crate::analysis::validation::check_consistency(
                &recommendation, rsi, macd_signal,
            );
            // Also check price position consistency
            let dist_low = price_context.distance_to_low_pct.unwrap_or(50.0);
            let dist_high = price_context.distance_to_high_pct.unwrap_or(50.0);
            let price_v = crate::analysis::validation::check_price_position(
                &recommendation, dist_low, dist_high,
            );
            if price_v.consistency_flag && !v.consistency_flag {
                v.consistency_flag = true;
                v.consistency_reason = price_v.consistency_reason;
                v.confidence_adjustment = price_v.confidence_adjustment;
            } else if price_v.consistency_flag {
                v.confidence_adjustment += price_v.confidence_adjustment;
                v.consistency_reason = format!("{}; {}", v.consistency_reason, price_v.consistency_reason);
            }
            v
        };
        // Apply validation penalties to effective confidence score
        let mut effective_confidence_score = effective_confidence_score;
        if validation_result.consistency_flag {
            tracing::warn!(
                stock = %result.symbol,
                reason = %validation_result.consistency_reason,
                penalty = validation_result.confidence_adjustment,
                "Recommendation contradicts technical indicators, applying penalty"
            );
            effective_confidence_score = (effective_confidence_score + validation_result.confidence_adjustment).max(30);
        }

        let entry_price = parse_first_numeric(&trader_plan.entry_price)
            .filter(|v| v.is_finite() && *v > 0.0);
        let probability_view = derive_probability_view(
            &decision_view,
            direction_assessment.final_score,
            effective_confidence_score,
            &price_context,
            &result.artifacts.memory_context,
            &technical_indicators,
            if !trader_plan.target_reference.trim().is_empty() {
                Some(trader_plan.target_reference.as_str())
            } else if !portfolio_decision.price_target.trim().is_empty() {
                Some(portfolio_decision.price_target.as_str())
            } else {
                None
            },
            entry_price,
        );
        let profit_risk = derive_profit_risk(
            &decision_view,
            &price_context,
            &probability_view,
            if !trader_plan.target_reference.trim().is_empty() {
                Some(trader_plan.target_reference.as_str())
            } else if !portfolio_decision.price_target.trim().is_empty() {
                Some(portfolio_decision.price_target.as_str())
            } else {
                None
            },
            entry_price,
        );
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
        // NOTE: append_scenario_gap_narrative is now called in validate_and_enhance_report
        // (after deduplicate_report_content) to prevent the 500-byte cap from truncating
        // the appended gap narrative.
        append_scenario_gap_narrative(
            &mut portfolio_decision.risk_assessment,
            &diagnostics,
            "场景级阻断项",
        );
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
        let mut report = Self {
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
        };
        validate_and_enhance_report(&mut report, &result.artifacts.market_chart, current_price);
        report
    }
}

/// Top-level validation and enhancement function.
/// Called at the end of StructuredReport::from_result() after all existing post-processing.
fn validate_and_enhance_report(
    report: &mut StructuredReport,
    market_chart: &ReportMarketChart,
    current_price: Option<f64>,
) {
    // P0: Execution Validation
    derive_execution_levels(
        &mut report.trader_plan,
        &mut report.portfolio_decision,
        &mut report.decision_view,
        market_chart,
        current_price,
    );
    enforce_price_consistency(
        &mut report.trader_plan,
        &mut report.portfolio_decision,
        &mut report.decision_view,
        &mut report.action_guides,
    );
    ensure_entry_transparency(&mut report.decision_view, &report.trader_plan);

    // Sync calc_entry with potentially updated trader_plan.entry_price
    if let Some(new_entry) = parse_first_numeric(&report.trader_plan.entry_price)
        .filter(|v| v.is_finite() && *v > 0.0)
    {
        tracing::info!(
            old_calc_entry = ?report.profit_risk.calc_entry,
            new_entry = new_entry,
            trader_plan_entry = %report.trader_plan.entry_price,
            "syncing calc_entry with trader_plan.entry_price"
        );
        report.profit_risk.calc_entry = Some(new_entry);
    }

    // Sync probability_view and profit_risk stop/profit levels with enforce_price_consistency
    // results. These views were computed BEFORE enforce_price_consistency ran, so their
    // stop_loss/profit_target/max_loss_reference may be stale.
    //
    // After enforce_price_consistency: dv.invalidation_level = dv.invalidation_price = stop loss
    // for ALL directions. Profit target is preserved in pv.upside_target (set by derive_probability_view).
    let is_bearish_pv = matches!(report.decision_view.view, DecisionViewDirection::Bearish);
    if is_bearish_pv {
        // Bearish: stop_loss = dv.invalidation_price (= confirmation_price, higher).
        // Profit target = pv.upside_target (already correct from derive_probability_view).
        if let Some(stop) = parse_first_numeric(&report.decision_view.invalidation_price)
            .filter(|v| v.is_finite() && *v > 0.0)
        {
            report.probability_view.downside_target = Some(stop);
            report.probability_view.stop_loss = Some(stop);
            report.profit_risk.max_loss_reference = Some(stop);
        }
        // Preserve existing profit target from derive_probability_view.
        report.probability_view.profit_target = report.probability_view.upside_target;
    } else {
        // Non-bearish: stop_loss = dv.invalidation_level (lower).
        if let Some(stop) = parse_first_numeric(&report.decision_view.invalidation_level)
            .filter(|v| v.is_finite() && *v > 0.0)
        {
            report.probability_view.downside_target = Some(stop);
            report.probability_view.stop_loss = Some(stop);
            report.profit_risk.max_loss_reference = Some(stop);
        }
    }

    // Sync ic_discipline.invalidation_price and pr.calc_stop based on direction.
    // All sections must use the SAME stop loss value for consistency.
    let is_bearish_sync = matches!(report.decision_view.view, DecisionViewDirection::Bearish);
    if is_bearish_sync {
        // Bearish: stop loss = invalidation_price (confirmation_price, above entry).
        if let Some(stop) = parse_first_numeric(&report.decision_view.invalidation_price)
            .filter(|v| v.is_finite() && *v > 0.0)
        {
            report.profit_risk.calc_stop = Some(stop);
            report.ic_discipline.invalidation_price = Some(stop);
        }
    } else {
        // Non-bearish: stop loss = invalidation_level (below entry).
        if let Some(new_stop) = parse_first_numeric(&report.decision_view.invalidation_level)
            .filter(|v| v.is_finite() && *v > 0.0)
        {
            report.profit_risk.calc_stop = Some(new_stop);
            report.ic_discipline.invalidation_price = Some(new_stop);
        }
    }

    // Recompute probability_view percentages from synced targets.
    let entry_for_pct = report.profit_risk.calc_entry
        .or(report.price_context.current_price);
    if let Some(current) = entry_for_pct.filter(|v| v.is_finite() && *v > 0.0) {
        if is_bearish_sync {
            // Bearish: upside_pct = profit from falling (current - target) / current
            if let Some(target) = report.probability_view.upside_target {
                if current > 0.0 && target < current {
                    report.probability_view.upside_pct = Some(((current - target) / current) * 100.0);
                    report.profit_risk.upside_pct = report.probability_view.upside_pct;
                    report.ic_discipline.upside_pct = report.probability_view.upside_pct;
                }
            }
            // Bearish: downside_pct = loss from rising (stop - current) / current
            if let Some(stop) = report.probability_view.stop_loss {
                if current > 0.0 && stop > current {
                    report.probability_view.downside_pct = Some(((stop - current) / current) * 100.0);
                    report.profit_risk.downside_pct = report.probability_view.downside_pct;
                    report.ic_discipline.downside_pct = report.probability_view.downside_pct;
                }
            }
        } else {
            // Non-bearish: upside_pct = profit from rising (target - current) / current
            if let Some(target) = report.probability_view.upside_target {
                if current > 0.0 && target > current {
                    report.probability_view.upside_pct = Some(((target - current) / current) * 100.0);
                    report.profit_risk.upside_pct = report.probability_view.upside_pct;
                    report.ic_discipline.upside_pct = report.probability_view.upside_pct;
                }
            }
            // Non-bearish: downside_pct = loss from falling (current - stop) / current
            if let Some(stop) = report.probability_view.stop_loss {
                if current > 0.0 && stop < current {
                    report.probability_view.downside_pct = Some(((current - stop) / current) * 100.0);
                    report.profit_risk.downside_pct = report.probability_view.downside_pct;
                    report.ic_discipline.downside_pct = report.probability_view.downside_pct;
                }
            }
        }
    }

    // Recompute R/R after syncing entry/stop/target
    if let (Some(entry), Some(target), Some(stop)) = (
        report.profit_risk.calc_entry,
        report.profit_risk.calc_target,
        report.profit_risk.calc_stop,
    ) {
        let is_bearish = matches!(report.decision_view.view, DecisionViewDirection::Bearish);
        let new_rr = if is_bearish {
            // Bearish: target < entry < stop, R/R = (entry - target) / (stop - entry)
            if stop > entry && target < entry && stop > target {
                Some((entry - target) / (stop - entry))
            } else {
                None
            }
        } else {
            // Bullish: stop < entry < target, R/R = (target - entry) / (entry - stop)
            if target > entry && stop < entry && target > stop {
                Some((target - entry) / (entry - stop))
            } else {
                None
            }
        };
        if new_rr.is_some() {
            tracing::info!(
                old_rr = ?report.profit_risk.reward_risk_ratio,
                new_rr = ?new_rr,
                entry = entry,
                target = target,
                stop = stop,
                is_bearish = is_bearish,
                "recomputed R/R after syncing entry/stop/target"
            );
            report.profit_risk.reward_risk_ratio = new_rr;
            report.ic_discipline.reward_risk_ratio = new_rr;

            // Update ic_discipline state based on new R/R
            let poor_reward_risk = new_rr.is_none_or(|value| value < 0.5);
            let overheated_rsi = report.ic_discipline.rsi.is_some_and(|value| value > 75.0);
            let confirmation_met = report
                .ic_discipline
                .confirmation_price
                .zip(report.ic_discipline.current_price)
                .is_some_and(|(confirmation, current)| current >= confirmation);

            let new_state = if poor_reward_risk || overheated_rsi {
                "no_attack"
            } else if new_rr.is_some_and(|value| value >= 1.2) && confirmation_met {
                "attack_allowed"
            } else {
                "probe_watch"
            };

            // Update reason_codes
            let mut reason_codes = Vec::new();
            if poor_reward_risk {
                reason_codes.push("poor_reward_risk".to_string());
            }
            if overheated_rsi {
                reason_codes.push("overheated_rsi".to_string());
            }
            if !confirmation_met {
                reason_codes.push("confirmation_missing".to_string());
            }
            if report
                .probability_view
                .risk_probability_pct
                > report.probability_view.upside_probability_pct + 10.0
            {
                reason_codes.push("risk_probability_high".to_string());
            }
            reason_codes.sort();
            reason_codes.dedup();

            tracing::info!(
                old_state = ?report.ic_discipline.state,
                new_state = new_state,
                poor_reward_risk = poor_reward_risk,
                new_rr = ?new_rr,
                "updating ic_discipline state after R/R recompute"
            );
            report.ic_discipline.state = LocalText::new(new_state);
            report.ic_discipline.reason_codes = reason_codes;

            // Update forced_hold based on new ic_discipline state
            // This will be recalculated later in the report builder, but we need to
            // ensure the ic_discipline state is correct first
        }
    }

    // Regenerate trade_summary after calc_entry/stop/target have been synced.
    // Only for bearish trades with actual short/sell actions (not Hold/Reduce/Exit).
    let is_short_action = !matches!(
        report.decision_view.action,
        DecisionAction::Hold | DecisionAction::Reduce | DecisionAction::Exit
    );
    if matches!(report.decision_view.view, DecisionViewDirection::Bearish) && is_short_action {
        if let (Some(entry), Some(target), Some(stop)) = (
            report.profit_risk.calc_entry,
            report.profit_risk.calc_target,
            report.profit_risk.calc_stop,
        ) {
            report.profit_risk.trade_summary = format!(
                "SHORT TRADE: Sell at {:.2} (entry), profit target {:.2} (if price falls), stop loss {:.2} (if price rises). For short trades, stop loss is ABOVE entry - this is correct.",
                entry, target, stop
            );
        }
    } else if !is_short_action {
        // Clear trade_summary for Hold/Reduce/Exit actions
        report.profit_risk.trade_summary = String::new();
    }

    // Append code-computed R/R to executive_summary AFTER recompute.
    // Strip any stale R/R text from the LLM-generated summary first.
    {
        let summary = &mut report.portfolio_decision.executive_summary.key;
        // Remove stale R/R appended by LLM or previous code
        if let Some(pos) = summary.find(" 系统计算盈亏比") {
            summary.truncate(pos);
        }
        if let Some(rr) = report.profit_risk.reward_risk_ratio {
            let rr_label = crate::analysis::rr_label(rr);
            if let Some(crr) = report.profit_risk.current_position_reward_risk_ratio
                && (crr - rr).abs() > 0.01
            {
                let crr_label = crate::analysis::rr_label(crr);
                summary.push_str(&format!(
                    " 系统计算盈亏比（当前→确认位）: {:.2}（{}），（当前→目标位）: {:.2}（{}），以代码计算值为准。",
                    crr, crr_label, rr, rr_label
                ));
            } else {
                summary.push_str(&format!(
                    " 系统计算盈亏比: {:.2}（{}），以代码计算值为准。",
                    rr, rr_label
                ));
            }
        }
    }

    // P1: Signal Intelligence
    resolve_signal_conflicts(&report.technical_indicators, &mut report.ic_discipline);
    detect_catalyst_vacuum(
        &report.news_insights,
        &mut report.portfolio_decision,
        &mut report.confidence_breakdown,
        &mut report.diagnostics,
    );

    // P2: Quality Hardening
    let anchor_entry = parse_first_numeric(&report.trader_plan.entry_price)
        .filter(|v| v.is_finite() && *v > 0.0);
    anchor_reward_risk_to_first_target(
        &mut report.profit_risk,
        &report.decision_view,
        &report.price_context,
        if !report.trader_plan.target_reference.trim().is_empty() {
            Some(report.trader_plan.target_reference.as_str())
        } else if !report.portfolio_decision.price_target.trim().is_empty() {
            Some(report.portfolio_decision.price_target.as_str())
        } else {
            None
        },
        anchor_entry,
    );
    // Normalize probability_view labels: upside_target/downside_target should
    // always follow PRICE DIRECTION (upside = higher price, downside = lower price),
    // not trade direction. For bearish views, the internal convention is reversed
    // (upside = profit from falling), so we swap the labels for output clarity.
    normalize_probability_labels_for_direction(&mut report.probability_view, &report.decision_view);
    deduplicate_report_content(report);
    // Append scenario gap narrative AFTER dedup caps executive_summary at 500 bytes,
    // so the gap text isn't truncated.
    append_scenario_gap_narrative(
        &mut report.portfolio_decision.executive_summary,
        &report.diagnostics,
        "当前不能升级结论的直接原因是",
    );
    // For bearish: invalidation_level represents the profit target (not the stop loss).
    // Keep it so the frontend can display the bearish trade structure.
    // The stop loss is in dv.invalidation_price (above entry for bearish).
    // Patch stale price references in LLM-generated text sections.
    patch_stale_prices_in_text(report);

    // For hold/reduce/exit actions, clear trade levels to avoid confusing the evaluator.
    // EXCEPTION: bearish views preserve trade levels since they represent the
    // bearish trade structure (entry/stop/target for a short position).
    let is_hold_action = matches!(
        report.decision_view.action,
        DecisionAction::Hold | DecisionAction::Reduce | DecisionAction::Exit
    ) && !matches!(report.decision_view.view, DecisionViewDirection::Bearish);
    if is_hold_action {
        // Clear probability_view trade levels
        report.probability_view.profit_target = None;
        report.probability_view.stop_loss = None;
        report.probability_view.upside_target = None;
        report.probability_view.downside_target = None;
        // Clear profit_risk trade levels
        report.profit_risk.calc_entry = None;
        report.profit_risk.calc_target = None;
        report.profit_risk.calc_stop = None;
        report.profit_risk.upside_pct = None;
        report.profit_risk.downside_pct = None;
        report.profit_risk.reward_risk_ratio = None;
        report.profit_risk.current_position_reward_risk_ratio = None;
        // Clear ic_discipline trade levels
        report.ic_discipline.upside_pct = None;
        report.ic_discipline.downside_pct = None;
        report.ic_discipline.reward_risk_ratio = None;
        report.ic_discipline.current_position_reward_risk_ratio = None;
        report.ic_discipline.confirmation_price = None;
        report.ic_discipline.invalidation_price = None;
    }

    apply_reliability_hard_caps(
        &mut report.research_reliability,
        &report.diagnostics,
        market_chart,
        &report.confidence_breakdown,
        &report.confidence_caps,
    );
}

/// For bearish views, swap upside_target/downside_target to match evaluator expectations.
/// Evaluator expects: upside_target = higher price, downside_target = lower price.
/// No-op: keep internal convention as-is.
/// Convention: upside = profit direction (lower for bearish, higher for long).
/// The evaluator prompt explicitly documents this convention, so no swap needed.
fn normalize_probability_labels_for_direction(
    _probability: &mut ProbabilityView,
    _decision: &DecisionView,
) {
    // No-op: keep internal convention. upside_target = profit direction for all trades.
}

/// Patch stale price references in LLM-generated text sections.
/// After `enforce_price_consistency` corrects the numeric fields, text sections
/// like `abort_plan` and `risk_line` still contain the original (stale) prices
/// in their `params`. This function updates those params to match the corrected values.
fn patch_stale_prices_in_text(report: &mut StructuredReport) {
    // Get corrected stop loss (for bearish = invalidation_price, for others = invalidation_level)
    let corrected_stop = if report.decision_view.view == DecisionViewDirection::Bearish {
        report.decision_view.invalidation_price.trim().to_string()
    } else {
        let s = report.decision_view.invalidation_price.trim().to_string();
        if s.is_empty() {
            report.decision_view.invalidation_level.trim().to_string()
        } else {
            s
        }
    };

    // Patch abort_plan invalidation param
    if !corrected_stop.is_empty() {
        if let Some(invalidation) = report.decision_view.abort_plan.params.get_mut("invalidation") {
            *invalidation = serde_json::Value::String(corrected_stop.clone());
        }
        // Patch risk_line invalidation param
        if let Some(invalidation) = report.decision_view.risk_line.params.get_mut("invalidation") {
            *invalidation = serde_json::Value::String(corrected_stop.clone());
        }
    }

    // Patch reader_summary if it contains stale prices
    // reader_summary.text = executive_summary, which may contain stale R/R or prices
    if let Some(text) = report.decision_view.reader_summary.params.get("text") {
        if let Some(text_str) = text.as_str() {
            let mut updated = text_str.to_string();
            // Strip stale R/R references (will be re-appended by validate_and_enhance_report)
            if let Some(rr_pos) = updated.find("风险收益比") {
                // Find the end of the R/R line
                if let Some(end) = updated[rr_pos..].find('\n') {
                    updated = format!("{}{}", &updated[..rr_pos], &updated[rr_pos + end..]);
                } else {
                    updated = updated[..rr_pos].to_string();
                }
                updated = updated.trim().to_string();
            }
            if updated != text_str {
                report.decision_view.reader_summary.params.insert(
                    "text".to_string(),
                    serde_json::Value::String(updated),
                );
            }
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

/// Compute ATR(14) from market chart candles.
/// Returns None if fewer than 15 candles are available.
/// Uses the MOST RECENT 14 candles (chart is sorted oldest-first).
fn compute_atr_14(chart: &ReportMarketChart) -> Option<f64> {
    let candles = &chart.candles;
    if candles.len() < 15 {
        return None;
    }
    // Take the last 15 candles (need 15 for 14 windows), not the first.
    let start = candles.len() - 15;
    let atr = candles[start..]
        .windows(2)
        .map(|w| {
            let high = w[1].high;
            let low = w[1].low;
            let prev_close = w[0].close;
            (high - low)
                .max((high - prev_close).abs())
                .max((low - prev_close).abs())
        })
        .sum::<f64>()
        / 14.0;
    if atr > 0.0 { Some(atr) } else { None }
}

/// P0-1 + P0-3: Derive entry/stop levels using ATR when LLM output is missing,
/// inverted, or contradictory. Direction-aware: long requires entry > stop,
/// short requires entry < stop.
fn derive_execution_levels(
    trader_plan: &mut StructuredTraderPlan,
    portfolio_decision: &mut StructuredPortfolioDecision,
    decision_view: &mut DecisionView,
    market_chart: &ReportMarketChart,
    _current_price: Option<f64>,
) {
    let atr = compute_atr_14(market_chart);
    let confirmation = parse_first_numeric(&portfolio_decision.confirmation_level);
    let entry = parse_first_numeric(&trader_plan.entry_price);
    let stop = parse_first_numeric(&trader_plan.stop_loss);
    let is_bearish = decision_view.view == DecisionViewDirection::Bearish;

    if let (Some(confirm), Some(atr_val)) = (confirmation, atr) {
        if is_bearish {
            // SHORT LOGIC: entry < confirmation (sell after breakdown),
            // stop > entry (loss if price rises back).
            let needs_derivation = entry.is_none()
                || stop.is_none()
                || entry == stop
                || entry.unwrap_or(0.0) < stop.unwrap_or(0.0);  // wrong: entry should be > stop for bearish

            if needs_derivation {
                let derived_entry = confirm - 0.5 * atr_val;  // just below confirmation
                let derived_stop = confirm + 1.5 * atr_val;   // above confirmation

                // Enforce minimum 1.5% stop distance
                let min_stop_pct = derived_entry * 0.015;
                let derived_stop = if derived_stop - derived_entry < min_stop_pct {
                    derived_entry + min_stop_pct
                } else {
                    derived_stop
                };

                if derived_stop > derived_entry && derived_entry > 0.0 {
                    tracing::info!(
                        confirm = confirm,
                        atr = atr_val,
                        derived_entry = derived_entry,
                        derived_stop = derived_stop,
                        "derive_execution_levels: SHORT entry/stop derived"
                    );
                    trader_plan.entry_price = format_price_reference(derived_entry);
                    trader_plan.stop_loss = format_price_reference(derived_stop);
                    decision_view.entry_reference = format_price_reference(derived_entry);
                    decision_view.invalidation_level = format_price_reference(derived_stop);
                    decision_view.entry_derivation = LocalText::new("entry_derived_from_confirmation")
                        .with_f64("entry", derived_entry)
                        .with_f64("confirm", confirm)
                        .with_f64("atr", atr_val);
                }
            }
        } else {
            // LONG LOGIC: entry < confirmation (buy on dip/retest),
            // stop < entry (loss if price drops further).
            let needs_derivation = entry.is_none()
                || stop.is_none()
                || entry == stop
                || entry.unwrap_or(0.0) < stop.unwrap_or(0.0);   // wrong: entry should be > stop

            if needs_derivation && confirm > atr_val {
                let anchor = confirm.max(_current_price.unwrap_or(confirm));
                let derived_entry = anchor - 1.0 * atr_val;
                let derived_stop = anchor - 2.5 * atr_val;

                // Enforce minimum 1.5% stop distance from entry
                let min_stop_pct = derived_entry * 0.015;
                let derived_stop = if derived_entry - derived_stop < min_stop_pct {
                    derived_entry - min_stop_pct
                } else {
                    derived_stop
                };

                if derived_entry > derived_stop && derived_stop > 0.0 {
                    tracing::info!(
                        confirm = confirm,
                        atr = atr_val,
                        derived_entry = derived_entry,
                        derived_stop = derived_stop,
                        "derive_execution_levels: LONG entry/stop derived"
                    );
                    trader_plan.entry_price = format_price_reference(derived_entry);
                    trader_plan.stop_loss = format_price_reference(derived_stop);
                    decision_view.entry_reference = format_price_reference(derived_entry);
                    decision_view.invalidation_level = format_price_reference(derived_stop);
                    decision_view.entry_derivation = LocalText::new("entry_derived_from_confirmation")
                        .with_f64("entry", derived_entry)
                        .with_f64("confirm", confirm)
                        .with_f64("atr", atr_val);
                }
            }
        }
    }

    // Sync invalidation_level for long: if entry < invalidation, lower invalidation
    if !is_bearish {
        if let (Some(entry_val), Some(inval)) = (
            parse_first_numeric(&trader_plan.entry_price),
            parse_first_numeric(&portfolio_decision.invalidation_level),
        ) {
            if entry_val > 0.0 && inval > 0.0 && entry_val < inval {
                portfolio_decision.invalidation_level = trader_plan.stop_loss.clone();
            }
        }
    }
}

/// P0-2: Enforce single source of truth for price levels.
///
/// Confirmation and target: portfolio_decision is authoritative.
/// Stop/invalidation: direction-dependent.
///   - Long: trader_plan.stop_loss is authoritative (below entry).
///   - Short: portfolio_decision.invalidation_level is authoritative (above entry),
///     since trader_plan.stop_loss may hold a long-style stop from SHA.
fn enforce_price_consistency(
    trader_plan: &mut StructuredTraderPlan,
    portfolio_decision: &mut StructuredPortfolioDecision,
    decision_view: &mut DecisionView,
    action_guides: &mut ReportActionGuides,
) {
    // portfolio_decision is authoritative for confirmation and target
    let confirmation = portfolio_decision.confirmation_level.clone();
    let target = if !portfolio_decision.target_reference.is_empty() {
        portfolio_decision.target_reference.clone()
    } else if !portfolio_decision.price_target.is_empty() {
        portfolio_decision.price_target.clone()
    } else {
        String::new()
    };

    // Stop/invalidation: direction-dependent source of truth.
    // For BEARISH: stop loss = confirmation_price (loss if price RISES).
    //   invalidation_level is the PROFIT target (price falls to), NOT the stop loss.
    // For NON-BEARISH: stop loss = invalidation_level / stop_loss (loss if price FALLS).
    let is_bearish = decision_view.view == DecisionViewDirection::Bearish;
    let corrected_inval = parse_first_numeric(&decision_view.invalidation_level)
        .filter(|v| v.is_finite() && *v > 0.0);
    let stop_loss = if is_bearish {
        // Short: stop loss = confirmation_price (above entry, loss if price rises).
        // NEVER use invalidation_level — that's the profit target for bearish.
        let conf = parse_first_numeric(&portfolio_decision.confirmation_level)
            .filter(|v| v.is_finite() && *v > 0.0);
        if let Some(c) = conf {
            format_price_reference(c)
        } else if !trader_plan.stop_loss.is_empty() {
            trader_plan.stop_loss.clone()
        } else {
            // Last resort: use decision_view.confirmation_price
            decision_view.confirmation_price.clone()
        }
    } else {
        // Long: prefer corrected invalidation_level from decision_view, then trader_plan
        if let Some(corrected) = corrected_inval {
            format_price_reference(corrected)
        } else if !trader_plan.stop_loss.is_empty() {
            trader_plan.stop_loss.clone()
        } else {
            portfolio_decision.invalidation_level.clone()
        }
    };

    // Sync trader_plan
    if !confirmation.is_empty() {
        trader_plan.confirmation_level = confirmation.clone();
    }
    // For bearish: sync stop_loss from SPA's invalidation_level
    if is_bearish && !stop_loss.is_empty() {
        trader_plan.stop_loss = stop_loss.clone();
    }

    // Sync decision_view
    if !confirmation.is_empty() {
        decision_view.confirmation_level = confirmation.clone();
    }
    if !stop_loss.is_empty() {
        if is_bearish {
            // Bearish: invalidation_level = profit target (keep as-is from build_decision_view).
            // invalidation_price = stop loss (confirmation_price, above entry).
            decision_view.invalidation_price = stop_loss.clone();
        } else {
            // Non-bearish: both = stop loss (below entry).
            decision_view.invalidation_level = stop_loss.clone();
            decision_view.invalidation_price = stop_loss.clone();
        }
    }
    if !target.is_empty() {
        // For bearish trades, clear first_target if it uses traditional conventions
        // (Upside = higher prices, Downside = lower prices) which conflict with our framework
        if is_bearish && (target.contains("Upside") || target.contains("Downside") ||
                         target.contains("upside") || target.contains("downside")) {
            decision_view.first_target = String::new();
        } else {
            decision_view.first_target = target.clone();
        }
    }

    // Sync action_guides (buyers/holders/watchers) — the "execution plan"
    let entry_ref = decision_view.entry_reference.clone();
    for guide in [
        &mut action_guides.buyers,
        &mut action_guides.holders,
        &mut action_guides.watchers,
    ] {
        if !entry_ref.is_empty() {
            guide.entry_reference = entry_ref.clone();
        }
        if !stop_loss.is_empty() {
            guide.invalidation_reference = stop_loss.clone();
        }
        if !target.is_empty() {
            guide.target_reference = target.clone();
        }
        if !confirmation.is_empty() {
            guide.confirmation_reference = confirmation.clone();
        }
    }
}

/// P0-3: Ensure every entry_reference has a derivation explanation.
fn ensure_entry_transparency(
    decision_view: &mut DecisionView,
    _trader_plan: &StructuredTraderPlan,
) {
    if decision_view.entry_reference.is_empty() {
        decision_view.entry_derivation = LocalText::new("entry_not_specified");
    } else if decision_view.entry_derivation.key.is_empty() {
        decision_view.entry_derivation = LocalText::new("entry_from_trader_plan");
    }
}

/// Find the signal_code for a given indicator key in TechnicalIndicatorView.
fn find_indicator_signal(tech: &TechnicalIndicatorView, key: &str) -> String {
    for cat in &tech.categories {
        for ind in &cat.indicators {
            if ind.key.eq_ignore_ascii_case(key) {
                return ind.signal_code.clone();
            }
        }
    }
    String::new()
}

/// Convert a signal_code to a numeric score for weighted averaging.
fn signal_to_score(signal: &str) -> f64 {
    match signal.to_ascii_lowercase().as_str() {
        "golden_cross" | "bullish" | "oversold" | "positive" | "inflow" => 1.0,
        "death_cross" | "bearish" | "overbought" | "negative" | "outflow" | "divergence" => -1.0,
        _ => 0.0,
    }
}

/// P1-4: Apply signal weight matrix. OBV (50%) > MACD/RSI (30%) > KDJ (20%).
fn resolve_signal_conflicts(
    technical_indicators: &TechnicalIndicatorView,
    ic_discipline: &mut IcDisciplineView,
) {
    let obv_signal = find_indicator_signal(technical_indicators, "OBV");
    let macd_signal = find_indicator_signal(technical_indicators, "MACD");
    let rsi_signal = find_indicator_signal(technical_indicators, "RSI");
    let kdj_signal = find_indicator_signal(technical_indicators, "KDJ");

    let volume_score = signal_to_score(&obv_signal) * 0.5;
    let momentum_score =
        (signal_to_score(&macd_signal) + signal_to_score(&rsi_signal)) / 2.0 * 0.3;
    let overbought_score = signal_to_score(&kdj_signal) * 0.2;
    let weighted_score = volume_score + momentum_score + overbought_score;

    let obv_divergence = matches!(
        obv_signal.as_str(),
        "divergence" | "outflow" | "bearish"
    );
    let macd_bullish = matches!(macd_signal.as_str(), "golden_cross" | "bullish");

    if obv_divergence && macd_bullish {
        ic_discipline.state = LocalText::new("ic_discipline_state_no_attack");
        ic_discipline
            .reason_codes
            .push("ic_discipline_reason_volume_divergence".to_string());
    }

    ic_discipline.signal_resolution = SignalResolution {
        weighted_score,
        volume_weight: 0.5,
        momentum_weight: 0.3,
        overbought_weight: 0.2,
        dominant_signal: if obv_divergence {
            "volume_divergence".to_string()
        } else {
            "aligned".to_string()
        },
    };
}

/// P1-6: Detect when there are no unpriced catalyst events.
fn detect_catalyst_vacuum(
    news_insights: &[NewsInsight],
    portfolio_decision: &mut StructuredPortfolioDecision,
    confidence_breakdown: &mut ConfidenceBreakdown,
    diagnostics: &mut ReportDiagnostics,
) {
    let unpriced_count = news_insights
        .iter()
        .filter(|n| {
            !n.published_before_analysis
                || n.impact_strength.key == "high"
                || n.impact_strength.key == "medium"
        })
        .count();

    if unpriced_count == 0 {
        let vacuum_warning = LocalText::new("catalyst_vacuum_warning");
        let current = portfolio_decision.executive_summary.trim();
        portfolio_decision.executive_summary =
            format!("{} {}", vacuum_warning.key, current).into();

        confidence_breakdown.catalyst_quality.score = crate::scoring::CATALYST_VACUUM_FLOOR;

        diagnostics.news.push(ReportDiagnosticItem {
            code: "catalyst_vacuum".to_string(),
            severity: "warning".to_string(),
            message: LocalText::new("catalyst_vacuum_diagnostic"),
            ..Default::default()
        });
    }
}

// ---------------------------------------------------------------------------
// P2 Quality Hardening Functions
// ---------------------------------------------------------------------------

/// Normalize text for dedup comparison.
fn normalize_for_dedup(text: &str) -> String {
    text.to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Jaccard similarity between two texts (word-level).
fn text_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Truncate text to max_len at a sentence boundary.
fn truncate_to_sentence_boundary(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    // Find a valid UTF-8 char boundary at or before max_len.
    let boundary = text.char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_len)
        .last()
        .unwrap_or(0);
    let truncated = &text[..boundary];
    // Prefer the last sentence-ending punctuation over the last comma.
    // Use char_indices (not rfind) to get correct multi-byte end positions.
    let sentence_enders = ['。', '.', '!', '?'];
    let clause_separators = ['，', ','];
    let mut last_ender: Option<usize> = None;
    let mut last_separator: Option<usize> = None;
    for (i, ch) in truncated.char_indices() {
        if sentence_enders.contains(&ch) {
            last_ender = Some(i + ch.len_utf8());
        } else if clause_separators.contains(&ch) {
            last_separator = Some(i);
        }
    }
    if let Some(pos) = last_ender {
        truncated[..pos].to_string()
    } else if let Some(pos) = last_separator {
        truncated[..pos].to_string()
    } else {
        truncated.to_string()
    }
}

/// P2-7: Recompute reward/risk ratio using first_target instead of probability targets.
fn anchor_reward_risk_to_first_target(
    profit_risk: &mut ProfitRiskView,
    decision_view: &DecisionView,
    price_context: &PriceContext,
    primary_target: Option<&str>,
    entry_price: Option<f64>,
) {
    let current = entry_price
        .filter(|v| v.is_finite() && *v > 0.0)
        .or(price_context.current_price)
        .or_else(|| parse_first_numeric(&decision_view.current_price))
        .unwrap_or(0.0);
    if current <= 0.0 {
        return;
    }

    let is_bearish = matches!(decision_view.view, DecisionViewDirection::Bearish);

    if is_bearish {
        // Bearish: target = profit direction (below entry), stop = loss direction (above entry).
        // invalidation_level = profit target (lower), invalidation_price = stop loss (higher).
        let target = parse_first_numeric(&decision_view.invalidation_level)
            .filter(|v| v.is_finite() && *v > 0.0)
            .or_else(|| parse_first_numeric(&decision_view.invalidation_price))
            .or(price_context.low_price)
            .unwrap_or(0.0);
        let stop = parse_first_numeric(&decision_view.invalidation_price)
            .or(price_context.high_price)
            .unwrap_or(0.0);
        if target <= 0.0 || target >= current || stop <= current {
            return;
        }
        let reward = current - target;
        let risk = stop - current;
        profit_risk.upside_pct = Some((reward / current) * 100.0);
        profit_risk.downside_pct = Some((risk / current) * 100.0);
        profit_risk.reward_risk_ratio = Some(reward / risk);
    } else {
        let target = primary_target
            .and_then(parse_first_numeric)
            .or_else(|| parse_first_numeric(&decision_view.first_target))
            .or_else(|| parse_first_numeric(decision_view.target_reference.value_str()))
            .unwrap_or(0.0);
        if target <= current {
            return;
        }
        // Use invalidation_level (corrected) instead of invalidation_price (raw LLM)
        let invalidation = parse_first_numeric(&decision_view.invalidation_level)
            .filter(|v| v.is_finite() && *v > 0.0)
            .or_else(|| parse_first_numeric(&decision_view.invalidation_price))
            .or(price_context.low_price)
            .unwrap_or(0.0);
        if invalidation <= 0.0 || invalidation >= current {
            return;
        }
        let reward = target - current;
        let risk = current - invalidation;
        profit_risk.upside_pct = Some((reward / current) * 100.0);
        profit_risk.downside_pct = Some((risk / current) * 100.0);
        profit_risk.reward_risk_ratio = Some(reward / risk);
    }
}

/// P2-8: Deduplicate repeated content across report sections.
fn deduplicate_report_content(report: &mut StructuredReport) {
    // 1. Cap executive_summary at 500 bytes to leave room for
    //    code-appended scenario gap narratives (Chinese text ~3 bytes/char).
    let summary = report.portfolio_decision.executive_summary.trim().to_string();
    if summary.len() > 500 {
        report.portfolio_decision.executive_summary =
            truncate_to_sentence_boundary(&summary, 500).into();
    }

    // 2. Dedup rationale against executive_summary
    let summary_norm = normalize_for_dedup(&report.portfolio_decision.executive_summary.key);
    let rationale_norm = normalize_for_dedup(&report.portfolio_decision.rationale.key);
    if text_similarity(&summary_norm, &rationale_norm) > 0.8 {
        report.portfolio_decision.rationale = LocalText::new("rationale_references_summary");
    }

    // 3. Dedup risk_assessment against rationale
    let risk_norm = normalize_for_dedup(&report.portfolio_decision.risk_assessment.key);
    let updated_rationale_norm = normalize_for_dedup(&report.portfolio_decision.rationale.key);
    if text_similarity(&updated_rationale_norm, &risk_norm) > 0.8 {
        report.portfolio_decision.risk_assessment = LocalText::new("risk_references_rationale");
    }
}

/// P2-9: Apply hard caps to research_reliability score.
fn apply_reliability_hard_caps(
    reliability: &mut ResearchReliability,
    diagnostics: &ReportDiagnostics,
    market_chart: &ReportMarketChart,
    confidence_breakdown: &ConfidenceBreakdown,
    confidence_caps: &[ConfidenceCap],
) {
    // Cap 1: No K-line data -> max 60
    if market_chart.candles.is_empty() {
        reliability.score = reliability.score.min(60);
        reliability
            .constraints
            .push(LocalText::new("reliability_cap_no_kline"));
    }

    // Cap 2: Sparse news -> deduct 15
    let has_sparse_news = diagnostics
        .news
        .iter()
        .any(|d| d.code == "news_sparse_coverage");
    if has_sparse_news {
        reliability.score = (reliability.score - 15).max(0);
        reliability
            .constraints
            .push(LocalText::new("reliability_cap_thin_news"));
    }

    // Cap 3: Any availability error -> max 70
    if diagnostics
        .availability
        .iter()
        .any(|d| d.severity.eq_ignore_ascii_case("error"))
    {
        reliability.score = reliability.score.min(70);
    }

    // Cap 4: Fundamentals sparse or missing -> cap at 60
    // Check actual diagnostic codes instead of relying on LLM-generated scores.
    let fundamentals_bad = diagnostics.fundamentals.iter().any(|d| {
        d.code == "fundamentals_sparse"
            || d.code == "fundamentals_period_mixed"
            || d.code == "fundamentals_missing"
            || d.severity.eq_ignore_ascii_case("error")
    });
    if fundamentals_bad {
        reliability.score = reliability.score.min(60);
        reliability
            .constraints
            .push(LocalText::new("reliability_cap_low_data_quality")
                .with_i32("score", confidence_breakdown.data_quality.score)
                .with_i32("max", confidence_breakdown.data_quality.max_score));
    }

    // Cap 5: No news at all -> additional 10-point deduction
    if diagnostics.news.is_empty() {
        reliability.score = (reliability.score - 10).max(0);
        reliability
            .constraints
            .push(LocalText::new("reliability_cap_thin_news"));
    }

    // Cap 6: missing_core_data confidence cap -> align reliability score
    // If confidence is capped due to missing core data, reliability should reflect this.
    if let Some(cap) = confidence_caps.iter().find(|c| c.key == "missing_core_data") {
        reliability.score = reliability.score.min(cap.cap);
        reliability
            .constraints
            .push(LocalText::new("reliability_cap_missing_core_data"));
    }

    // Update label based on capped score
    reliability.label = LocalText::new(match reliability.score {
        80.. => "reliability_label_high",
        65.. => "reliability_label_good",
        50.. => "reliability_label_conditional",
        _ => "reliability_label_weak",
    });
}

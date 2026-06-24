use crate::engine::llm::parse::{DiagnosisIssue, IssueSeverity};
use crate::engine::task_manager::TaskRunParams;
use crate::models::{AnalysisResult, InvestmentDebateState, RiskDebateState};

fn compact_decision_context(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if text.is_empty() || max_chars == 0 {
        return String::new();
    }
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let priority_line = |line: &str| -> usize {
        let lower = line.to_ascii_lowercase();
        usize::from(lower.contains("recommend"))
            + usize::from(lower.contains("rating"))
            + usize::from(lower.contains("confidence"))
            + usize::from(lower.contains("risk"))
            + usize::from(lower.contains("trigger"))
            + usize::from(lower.contains("invalidation"))
            + usize::from(lower.contains("stop"))
            + usize::from(lower.contains("target"))
            + usize::from(lower.contains("entry"))
            + usize::from(lower.contains("price"))
            + usize::from(lower.contains("support"))
            + usize::from(lower.contains("resistance"))
            + usize::from(lower.contains("cash"))
            + usize::from(lower.contains("debt"))
            + usize::from(lower.contains("margin"))
            + usize::from(lower.contains("profit"))
            + usize::from(lower.contains("gap"))
            + usize::from(lower.contains("gap"))
            + usize::from(lower.contains("risk"))
            + usize::from(lower.contains("trigger"))
            + usize::from(lower.contains("stop-loss"))
            + usize::from(lower.contains("target"))
    };

    let mut selected = Vec::new();
    let mut used = 0usize;
    for line in lines.iter().filter(|line| priority_line(line) > 0) {
        let len = line.chars().count() + 1;
        if used + len > max_chars.saturating_sub(32) {
            break;
        }
        selected.push((*line).to_string());
        used += len;
        if selected.len() >= 12 {
            break;
        }
    }

    if selected.len() < 6 {
        for line in &lines {
            if selected.iter().any(|item| item == line) {
                continue;
            }
            let len = line.chars().count() + 1;
            if used + len > max_chars.saturating_sub(32) {
                break;
            }
            selected.push((*line).to_string());
            used += len;
            if selected.len() >= 12 {
                break;
            }
        }
    }

    if selected.is_empty() {
        let chars = text.chars().take(max_chars).collect::<String>();
        return format!("{chars}\n...[truncated]");
    }

    let mut compact = selected.join("\n");
    if compact.chars().count() > max_chars {
        compact = compact.chars().take(max_chars).collect::<String>();
    }
    compact
}
impl crate::TaskManager {
    fn research_manager_needs_deep_llm(result: &AnalysisResult, params: &TaskRunParams) -> bool {
        if crate::engine::config::analysis_debug_quick_only() {
            return false;
        }
        let report = &result.report;
        let memory = &params.memory_context;
        let user = &params.user_context;
        let confidence = report.confidence_score;
        let action = report.action_score;
        let direction_abs = report.direction_score.abs();
        let reward_risk = report.profit_risk.reward_risk_ratio;
        let setup_history_weak = memory.setup_resolved_match_count > 0
            && (memory.setup_resolved_match_count < 2
                || memory.setup_match_hit_rate < 0.5
                || memory.setup_match_avg_alpha_return <= 0.0);
        let directional_conflict = memory.setup_resolved_match_count >= 2
            && ((report.direction_score > 20
                && memory.setup_short_match_count > memory.setup_long_match_count)
                || (report.direction_score < -20
                    && memory.setup_long_match_count > memory.setup_short_match_count));
        let boundary_case = (45..=70).contains(&confidence)
            || (40..=60).contains(&action)
            || (15..=35).contains(&direction_abs)
            || reward_risk.is_some_and(|value| (0.7..=1.3).contains(&value));
        let capital_impact = matches!(
            user.position_state.as_str(),
            "holding" | "bought" | "already_bought"
        ) || user.holding_ratio_pct.is_some_and(|value| value >= 20.0);
        let incomplete_but_actionable = !report.execution_readiness.execution_boundary_complete
            && direction_abs >= 25
            && confidence >= 45;
        setup_history_weak
            || directional_conflict
            || boundary_case
            || capital_impact
            || incomplete_but_actionable
    }

    pub(crate) async fn run_bull_researcher_node(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        quick_llm: &crate::engine::llm::LlmClient,
    ) -> anyhow::Result<()> {
        let debate_turns = result.graph.investment_debate.turns.clone();
        let round = debate_turns.len() / 2;
        if debate_turns.len() >= self.max_debate_rounds * 2 || debate_turns.len() > round * 2 {
            return Ok(());
        }
        self.update_graph_stage(
            &result.task_id,
            91,
            "Bull/Bear Debate",
            "Bull Researcher Speaking",
            "Bull Researcher Speaking",
        )
        .await?;
        let mut bull_history = result.graph.investment_debate.bull_history.clone();
        let bear_history = result.graph.investment_debate.bear_history.clone();
        let bull_turn = quick_llm
            .generate_debate_turn(crate::engine::llm::DebateTurnParams {
                symbol: &result.symbol,
                market_type: &params.market_type,
                analysis_date: &params.analysis_date,
                speaker: "Bull Researcher",
                stance: "bull",
                mission: "Build the strongest bull case, emphasizing upside catalysts, expectation gaps, odds, and acceleration paths.",
                context_sections: &[
                    ("Market Technical", &result.agent_state.market_report),
                    ("Fundamentals", &result.agent_state.fundamentals_report),
                    ("News Events", &result.agent_state.news_report),
                    ("Sentiment", &result.agent_state.sentiment_report),
                    ("Past Context", &params.past_context),
                    ("Bear History", &bear_history),
                ],
                retry_hint: None,
            })
            .await?;
        bull_history.push_str(&format!(
            "\n\n[Round {}]\n{}",
            round + 1,
            bull_turn.response
        ));
        let mut turns = debate_turns;
        turns.push(crate::engine::analysis::graph::debate_turn_from_generated(
            &bull_turn,
        ));
        result.graph.investment_debate = InvestmentDebateState {
            bull_history: bull_history.trim().to_string(),
            bear_history: bear_history.trim().to_string(),
            history: format!(
                "Bull Researcher:\n{}\n\nBear Researcher:\n{}",
                bull_history.trim(),
                bear_history.trim()
            ),
            current_response: bull_turn.response.clone(),
            judge_decision: result.graph.investment_debate.judge_decision.clone(),
            count: turns.len() as i32,
            turns,
        };
        result.agent_state.investment_debate_state = result.graph.investment_debate.clone();
        result.sync_derived_fields();
        result.artifacts.llm_token_usage = quick_llm.usage_summary().await;
        self.persist_runtime_stage(
            result,
            &format!("debate:bull:{}", round + 1),
            "Bull Researcher",
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn run_bear_researcher_node(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        quick_llm: &crate::engine::llm::LlmClient,
    ) -> anyhow::Result<()> {
        let debate_turns = result.graph.investment_debate.turns.clone();
        let round = debate_turns.len() / 2;
        if debate_turns.len() >= self.max_debate_rounds * 2 || debate_turns.len() != round * 2 + 1 {
            return Ok(());
        }
        self.update_graph_stage(
            &result.task_id,
            92,
            "Bull/Bear Debate",
            "Bear Researcher Speaking",
            "Bear Researcher Speaking",
        )
        .await?;
        let bull_history = result.graph.investment_debate.bull_history.clone();
        let mut bear_history = result.graph.investment_debate.bear_history.clone();
        let bear_turn = quick_llm
            .generate_debate_turn(crate::engine::llm::DebateTurnParams {
                symbol: &result.symbol,
                market_type: &params.market_type,
                analysis_date: &params.analysis_date,
                speaker: "Bear Researcher",
                stance: "bear",
                mission: "Build the strongest bear case, emphasizing fragile assumptions, valuation compression, earnings misses, and liquidity risks.",
                context_sections: &[
                    ("Market Technical", &result.agent_state.market_report),
                    ("Fundamentals", &result.agent_state.fundamentals_report),
                    ("News Events", &result.agent_state.news_report),
                    ("Sentiment", &result.agent_state.sentiment_report),
                    ("Past Context", &params.past_context),
                    ("Bull History", &bull_history),
                ],
                retry_hint: None,
            })
            .await?;
        bear_history.push_str(&format!(
            "\n\n[Round {}]\n{}",
            round + 1,
            bear_turn.response
        ));
        let mut turns = debate_turns;
        turns.push(crate::engine::analysis::graph::debate_turn_from_generated(
            &bear_turn,
        ));
        result.graph.investment_debate = InvestmentDebateState {
            bull_history: bull_history.trim().to_string(),
            bear_history: bear_history.trim().to_string(),
            history: format!(
                "Bull Researcher:\n{}\n\nBear Researcher:\n{}",
                bull_history.trim(),
                bear_history.trim()
            ),
            current_response: bear_turn.response.clone(),
            judge_decision: result.graph.investment_debate.judge_decision.clone(),
            count: turns.len() as i32,
            turns,
        };
        result.agent_state.investment_debate_state = result.graph.investment_debate.clone();
        result.sync_derived_fields();
        result.artifacts.llm_token_usage = quick_llm.usage_summary().await;
        if result.graph.investment_debate.count >= (self.max_debate_rounds as i32 * 2)
            && !result
                .graph
                .checkpoints
                .iter()
                .any(|item| item.stage_key == "investment_debate")
        {
            crate::engine::analysis::graph::push_checkpoint(
                result,
                "investment_debate",
                "Bull/Bear Debate",
                "completed",
                "Bull/Bear Debate Completed".to_string(),
            );
            self.persist_runtime_stage(result, "debate", "Research Manager")
                .await?;
        }
        self.persist_runtime_stage(
            result,
            &format!("debate:bear:{}", round + 1),
            "Bear Researcher",
        )
        .await?;
        Ok(())
    }
}
impl crate::TaskManager {
    /// Run a single risk discussion round with all 3 analysts in parallel.
    pub(crate) async fn run_risk_round(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        quick_llm: &crate::engine::llm::LlmClient,
    ) -> anyhow::Result<()> {
        let risk_turns = result.graph.risk_debate.turns.clone();
        let round = risk_turns.len() / 3;
        if risk_turns.len() >= self.max_risk_discuss_rounds * 3 {
            return Ok(());
        }

        self.update_graph_stage(
            &result.task_id,
            96,
            "Risk Management Debate",
            &format!(
                "Round {} — Three risk analysts speaking in parallel",
                round + 1
            ),
            &format!("Risk Discussion Round {}", round + 1),
        )
        .await?;

        let aggressive_history = result.graph.risk_debate.aggressive_history.clone();
        let conservative_history = result.graph.risk_debate.conservative_history.clone();
        let neutral_history = result.graph.risk_debate.neutral_history.clone();

        // Fire all 3 LLM calls concurrently
        let symbol = result.symbol.clone();
        let market_type = params.market_type.clone();
        let analysis_date = params.analysis_date.clone();
        let investment_plan = result.agent_state.investment_plan.clone();
        let trader_plan = result.agent_state.trader_investment_plan.clone();
        let past_context = params.past_context.clone();
        let llm_a = quick_llm.clone();
        let llm_c = quick_llm.clone();
        let llm_n = quick_llm.clone();

        let (aggressive, conservative, neutral) = tokio::join!(
            async {
                llm_a.generate_debate_turn(crate::engine::llm::DebateTurnParams {
                    symbol: &symbol,
                    market_type: &market_type,
                    analysis_date: &analysis_date,
                    speaker: "Aggressive Analyst",
                    stance: "aggressive",
                    mission: "Take the position of an aggressive risk-taker, emphasizing odds, timing, position utilization, and high-return windows.",
                    context_sections: &[
                        ("Research Manager", investment_plan.as_str()),
                        ("Trader", trader_plan.as_str()),
                        ("Past Context", past_context.as_str()),
                        ("Neutral History", neutral_history.as_str()),
                    ],
                    retry_hint: None,
                }).await
            },
            async {
                llm_c.generate_debate_turn(crate::engine::llm::DebateTurnParams {
                    symbol: &symbol,
                    market_type: &market_type,
                    analysis_date: &analysis_date,
                    speaker: "Conservative Analyst",
                    stance: "conservative",
                    mission: "Take the position of a defensive risk controller, emphasizing drawdowns, invalidation conditions, liquidity, execution discipline, and uncertainty.",
                    context_sections: &[
                        ("Research Manager", investment_plan.as_str()),
                        ("Trader", trader_plan.as_str()),
                        ("Aggressive History", aggressive_history.as_str()),
                    ],
                    retry_hint: None,
                }).await
            },
            async {
                llm_n.generate_debate_turn(crate::engine::llm::DebateTurnParams {
                    symbol: &symbol,
                    market_type: &market_type,
                    analysis_date: &analysis_date,
                    speaker: "Neutral Analyst",
                    stance: "neutral",
                    mission: "Take the position of a neutral risk coordinator, balancing odds and risk with more balanced risk language.",
                    context_sections: &[
                        ("Research Manager", investment_plan.as_str()),
                        ("Trader", trader_plan.as_str()),
                        ("Aggressive History", aggressive_history.as_str()),
                        ("Conservative History", conservative_history.as_str()),
                    ],
                    retry_hint: None,
                }).await
            }
        );

        let aggressive = aggressive?;
        let conservative = conservative?;
        let neutral = neutral?;

        // Apply results sequentially
        let mut agg_hist = aggressive_history;
        let mut cons_hist = conservative_history;
        let mut neut_hist = neutral_history;
        agg_hist.push_str(&format!(
            "\n\n[Round {}]\n{}",
            round + 1,
            aggressive.response
        ));
        cons_hist.push_str(&format!(
            "\n\n[Round {}]\n{}",
            round + 1,
            conservative.response
        ));
        neut_hist.push_str(&format!("\n\n[Round {}]\n{}", round + 1, neutral.response));

        let mut turns = risk_turns;
        turns.push(crate::engine::analysis::graph::debate_turn_from_generated(
            &aggressive,
        ));
        turns.push(crate::engine::analysis::graph::debate_turn_from_generated(
            &conservative,
        ));
        turns.push(crate::engine::analysis::graph::debate_turn_from_generated(
            &neutral,
        ));

        result.graph.risk_debate = RiskDebateState {
            aggressive_history: agg_hist.trim().to_string(),
            conservative_history: cons_hist.trim().to_string(),
            neutral_history: neut_hist.trim().to_string(),
            history: format!(
                "Aggressive Analyst:\n{}\n\nConservative Analyst:\n{}\n\nNeutral Analyst:\n{}",
                agg_hist.trim(),
                cons_hist.trim(),
                neut_hist.trim()
            ),
            latest_speaker: "Neutral Analyst".to_string(),
            current_aggressive_response: aggressive.response.clone(),
            current_conservative_response: conservative.response.clone(),
            current_neutral_response: neutral.response.clone(),
            judge_decision: result.graph.risk_debate.judge_decision.clone(),
            count: turns.len() as i32,
            turns,
        };
        result.agent_state.risk_debate_state = result.graph.risk_debate.clone();
        result.sync_derived_fields();
        result.artifacts.llm_token_usage = quick_llm.usage_summary().await;
        self.persist_runtime_stage(
            result,
            &format!("risk:round:{}", round + 1),
            "Risk Discussion",
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn run_research_manager_stage(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        quick_llm: &crate::engine::llm::LlmClient,
        deep_llm: &crate::engine::llm::LlmClient,
    ) -> anyhow::Result<()> {
        if result.agent_state.investment_plan.trim().is_empty() {
            self.update_graph_stage(
                &result.task_id,
                93,
                "Research Manager Decision",
                "Generating Research Manager Summary",
                "Research Manager Deciding",
            )
            .await?;
            let calibration_memo = crate::engine::llm::LlmClient::calibration_memo(
                &params.memory_context,
                &params.market_type,
                &params.analysis_date,
            );
            self.refresh_structured_report_snapshot(result).await?;
            let fact_sheet = super::facts::build_decision_fact_sheet(result);
            let fact_sheet = if params.user_context_prompt.trim().is_empty() {
                fact_sheet
            } else {
                format!(
                    "{fact_sheet}\n\nUser context:\n{}",
                    params.user_context_prompt
                )
            };
            result.artifacts.calibration_memo = calibration_memo.clone();
            let use_deep_llm = Self::research_manager_needs_deep_llm(result, params);
            tracing::info!(
                task_id = %result.task_id,
                symbol = %result.symbol,
                use_deep_llm,
                confidence_score = result.report.confidence_score,
                action_score = result.report.action_score,
                direction_score = result.report.direction_score,
                "selected model tier for research manager"
            );
            let research_llm = if use_deep_llm { deep_llm } else { quick_llm };
            let bull_ctx =
                compact_decision_context(&result.graph.investment_debate.bull_history, 1800);
            let bear_ctx =
                compact_decision_context(&result.graph.investment_debate.bear_history, 1800);
            let mut research_manager = None::<crate::engine::llm::GeneratedResearchManager>;
            let mut last_issues = Vec::new();
            for retry in 0..=2u32 {
                let hint = if retry == 0 {
                    None
                } else {
                    Some(crate::engine::llm::retry::default_retry_hint_builder(
                        &last_issues,
                        retry,
                    ))
                };
                let candidate = research_llm
                    .generate_research_manager(crate::engine::llm::ResearchManagerParams {
                        symbol: &result.symbol,
                        market_type: &params.market_type,
                        analysis_date: &params.analysis_date,
                        market_report: &result.agent_state.market_report,
                        fundamentals_report: &result.agent_state.fundamentals_report,
                        news_report: &result.agent_state.news_report,
                        sentiment_report: &result.agent_state.sentiment_report,
                        bull_case: &bull_ctx,
                        bear_case: &bear_ctx,
                        fact_sheet: &fact_sheet,
                        calibration_memo: &calibration_memo,
                        retry_hint: hint.as_deref(),
                    })
                    .await?;
                let issues = {
                    let mut v = Vec::new();
                    if candidate.rationale == "Model did not return research manager rationale." {
                        v.push(DiagnosisIssue::error(
                            "research_manager",
                            "rationale",
                            "rationale is default placeholder",
                        ));
                    }
                    if candidate.risk_assessment == "Model did not return risk assessment." {
                        v.push(DiagnosisIssue::error(
                            "research_manager",
                            "risk_assessment",
                            "risk_assessment is default placeholder",
                        ));
                    }
                    v
                };
                let has_errors = issues
                    .iter()
                    .any(|i| matches!(i.severity, IssueSeverity::Error));
                if !has_errors {
                    if retry > 0 {
                        tracing::info!(
                            stage = "research_manager",
                            retry,
                            "LLM output fixed after retry"
                        );
                    }
                    research_manager = Some(candidate);
                    break;
                }
                last_issues = issues;
                research_manager = Some(candidate);
                tracing::warn!(
                    stage = "research_manager",
                    retry,
                    issues = %last_issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(", "),
                    "LLM output has quality issues, retrying"
                );
            }
            let research_manager = research_manager.expect("at least one LLM attempt must succeed");
            result.agent_state.sender = "Research Manager".to_string();
            result.agent_state.investment_plan = research_manager.rendered_plan();
            result.agent_state.structured_research_plan = crate::models::StructuredResearchPlan {
                recommendation: research_manager.recommendation.clone().into(),
                confidence: research_manager.confidence_string().into(),
                risk_assessment: research_manager.risk_assessment.clone().into(),
                rationale: research_manager.rationale.clone().into(),
                strategic_actions: research_manager.strategic_actions.clone().into(),
                missing_evidence_ladder: crate::models::MissingEvidenceLadder {
                    tolerable_gaps: research_manager
                        .missing_evidence_ladder
                        .tolerable_gaps
                        .clone(),
                    manageable_gaps: research_manager
                        .missing_evidence_ladder
                        .manageable_gaps
                        .clone(),
                    blocking_gaps: research_manager
                        .missing_evidence_ladder
                        .blocking_gaps
                        .clone(),
                },
                trigger_checklist: research_manager.trigger_checklist.clone(),
                accounting_scope_hypothesis: research_manager
                    .accounting_scope_hypothesis
                    .clone()
                    .unwrap_or_default(),
                markdown: result.agent_state.investment_plan.clone(),
            };
            result.graph.investment_debate.judge_decision =
                result.agent_state.investment_plan.clone();
            result.sync_derived_fields();
            result.artifacts.llm_token_usage = research_llm.usage_summary().await;
            crate::engine::analysis::graph::push_checkpoint(
                result,
                "research_manager",
                "Research Manager",
                "completed",
                research_manager.rationale.clone(),
            );
            self.persist_runtime_stage(result, "research", "Research Manager")
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn run_trader_stage(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        quick_llm: &crate::engine::llm::LlmClient,
    ) -> anyhow::Result<()> {
        if result.agent_state.trader_investment_plan.trim().is_empty() {
            self.update_graph_stage(
                &result.task_id,
                94,
                "Trading Plan Generation",
                "Generating Trader Execution Plan",
                "Generating Trading Plan",
            )
            .await?;
            let calibration_memo = crate::engine::llm::LlmClient::calibration_memo(
                &params.memory_context,
                &params.market_type,
                &params.analysis_date,
            );
            self.refresh_structured_report_snapshot(result).await?;
            let fact_sheet = super::facts::build_decision_fact_sheet(result);
            let fact_sheet = if params.user_context_prompt.trim().is_empty() {
                fact_sheet
            } else {
                format!(
                    "{fact_sheet}\n\nUser context:\n{}",
                    params.user_context_prompt
                )
            };
            result.artifacts.calibration_memo = calibration_memo.clone();
            let plan_ctx = compact_decision_context(&result.agent_state.investment_plan, 1600);
            let bull_ctx =
                compact_decision_context(&result.graph.investment_debate.bull_history, 1200);
            let bear_ctx =
                compact_decision_context(&result.graph.investment_debate.bear_history, 1200);
            let summary = result.derived_summary();
            let mut trader = None::<crate::engine::llm::GeneratedTraderDecision>;
            let mut last_issues = Vec::new();
            for retry in 0..=2u32 {
                let hint = if retry == 0 {
                    None
                } else {
                    Some(crate::engine::llm::retry::default_retry_hint_builder(
                        &last_issues,
                        retry,
                    ))
                };
                let candidate = quick_llm
                    .generate_trader_decision(crate::engine::llm::TraderDecisionParams {
                        symbol: &result.symbol,
                        market_type: &params.market_type,
                        analysis_date: &params.analysis_date,
                        investment_plan: &plan_ctx,
                        bull_case: &bull_ctx,
                        bear_case: &bear_ctx,
                        research_summary: &summary,
                        fact_sheet: &fact_sheet,
                        calibration_memo: &calibration_memo,
                        retry_hint: hint.as_deref(),
                    })
                    .await?;
                let issues = {
                    let mut v = Vec::new();
                    if candidate.trader_plan == "Model did not return a trading plan."
                        || candidate.trader_plan.trim().is_empty()
                    {
                        v.push(DiagnosisIssue::error(
                            "trader_decision",
                            "trader_plan",
                            "trader_plan is default placeholder or empty",
                        ));
                    }
                    if candidate.reasoning == "Model did not return trading reasoning." {
                        v.push(DiagnosisIssue::error(
                            "trader_decision",
                            "reasoning",
                            "reasoning is default placeholder",
                        ));
                    }
                    v
                };
                let has_errors = issues
                    .iter()
                    .any(|i| matches!(i.severity, IssueSeverity::Error));
                if !has_errors {
                    if retry > 0 {
                        tracing::info!(stage = "trader", retry, "LLM output fixed after retry");
                    }
                    trader = Some(candidate);
                    break;
                }
                last_issues = issues;
                trader = Some(candidate);
                tracing::warn!(
                    stage = "trader",
                    retry,
                    issues = %last_issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(", "),
                    "LLM output has quality issues, retrying"
                );
            }
            let trader = trader.expect("at least one LLM attempt must succeed");
            result.agent_state.sender = "Trader".to_string();
            result.agent_state.trader_investment_plan = trader.trader_plan.clone();
            result.agent_state.structured_trader_plan = crate::models::StructuredTraderPlan {
                action: trader.action.clone().into(),
                raw_action: trader.action.clone(),
                calibrated_action: trader.action.clone(),
                reasoning: trader.reasoning.clone().into(),
                entry_price: trader
                    .entry_price
                    .as_ref()
                    .map(crate::engine::llm::parse::normalize_value)
                    .unwrap_or_default(),
                stop_loss: trader
                    .stop_loss
                    .as_ref()
                    .map(crate::engine::llm::parse::normalize_value)
                    .unwrap_or_default(),
                confirmation_level: trader
                    .confirmation_level
                    .as_ref()
                    .map(crate::engine::llm::parse::normalize_value)
                    .unwrap_or_default(),
                target_reference: trader.target_reference.clone().unwrap_or_default(),
                target_condition: trader.target_condition.clone().unwrap_or_default(),
                time_horizon: trader.time_horizon.clone().unwrap_or_default(),
                position_sizing: trader.position_sizing.clone().unwrap_or_default(),
                proposal: crate::models::LocalText::new(trader.action.trim().to_string()),
                execution_trigger_checklist: trader.execution_trigger_checklist.clone(),
                blocking_gaps: trader.blocking_gaps.clone(),
                time_stop_deadline: trader.time_stop_deadline.clone().unwrap_or_default(),
                time_stop_reason: trader.time_stop_reason.clone().unwrap_or_default(),
                markdown: result.agent_state.trader_investment_plan.clone(),
            };
            result.sync_derived_fields();
            result.artifacts.llm_token_usage = quick_llm.usage_summary().await;
            crate::engine::analysis::graph::push_checkpoint(
                result,
                "trader",
                "Trader",
                "completed",
                trader.reasoning.clone(),
            );
            self.persist_runtime_stage(result, "trader", "Trader")
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn run_portfolio_stage(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        deep_llm: &crate::engine::llm::LlmClient,
    ) -> anyhow::Result<()> {
        if result.agent_state.final_trade_decision.trim().is_empty() {
            self.update_graph_stage(
                &result.task_id,
                98,
                "Portfolio Manager Decision",
                "Generating Final Portfolio Conclusion and Report",
                "Portfolio Manager Deciding",
            )
            .await?;
            let calibration_memo = crate::engine::llm::LlmClient::calibration_memo(
                &params.memory_context,
                &params.market_type,
                &params.analysis_date,
            );
            self.refresh_structured_report_snapshot(result).await?;
            let fact_sheet = super::facts::build_decision_fact_sheet(result);
            let fact_sheet = if params.user_context_prompt.trim().is_empty() {
                fact_sheet
            } else {
                format!(
                    "{fact_sheet}\n\nUser context:\n{}",
                    params.user_context_prompt
                )
            };
            result.artifacts.calibration_memo = calibration_memo.clone();
            let invest_plan_ctx =
                compact_decision_context(&result.agent_state.investment_plan, 1400);
            let trader_plan_ctx =
                compact_decision_context(&result.agent_state.trader_investment_plan, 1200);
            let bull_ctx = format!(
                "{}\n\n{}",
                compact_decision_context(&result.graph.investment_debate.bull_history, 900),
                compact_decision_context(&result.graph.risk_debate.aggressive_history, 900)
            );
            let bear_ctx = format!(
                "{}\n\n{}\n\n{}",
                compact_decision_context(&result.graph.investment_debate.bear_history, 900),
                compact_decision_context(&result.graph.risk_debate.conservative_history, 800),
                compact_decision_context(&result.graph.risk_debate.neutral_history, 800)
            );
            let mut portfolio_decision = None::<crate::engine::llm::GeneratedPortfolioDecision>;
            let mut last_issues = Vec::new();
            for retry in 0..=2u32 {
                let hint = if retry == 0 {
                    None
                } else {
                    Some(crate::engine::llm::retry::default_retry_hint_builder(
                        &last_issues,
                        retry,
                    ))
                };
                let candidate = deep_llm
                    .generate_portfolio_decision(crate::engine::llm::PortfolioDecisionParams {
                        symbol: &result.symbol,
                        market_type: &params.market_type,
                        analysis_date: &params.analysis_date,
                        investment_plan: &invest_plan_ctx,
                        trader_plan: &trader_plan_ctx,
                        bull_case: &bull_ctx,
                        bear_case: &bear_ctx,
                        fact_sheet: &fact_sheet,
                        calibration_memo: &calibration_memo,
                        retry_hint: hint.as_deref(),
                    })
                    .await?;
                let issues = {
                    let mut v = Vec::new();
                    if candidate.executive_summary
                        == "Model did not return portfolio manager executive summary."
                    {
                        v.push(DiagnosisIssue::error(
                            "portfolio_decision",
                            "executive_summary",
                            "executive_summary is default placeholder",
                        ));
                    }
                    if candidate.rationale == "Model did not return research manager rationale."
                        || candidate.investment_thesis
                            == "Model did not return portfolio manager investment thesis."
                    {
                        v.push(DiagnosisIssue::error(
                            "portfolio_decision",
                            "rationale",
                            "rationale/investment_thesis is default placeholder",
                        ));
                    }
                    v
                };
                let has_errors = issues
                    .iter()
                    .any(|i| matches!(i.severity, IssueSeverity::Error));
                if !has_errors {
                    if retry > 0 {
                        tracing::info!(stage = "portfolio", retry, "LLM output fixed after retry");
                    }
                    portfolio_decision = Some(candidate);
                    break;
                }
                last_issues = issues;
                portfolio_decision = Some(candidate);
                tracing::warn!(
                    stage = "portfolio",
                    retry,
                    issues = %last_issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(", "),
                    "LLM output has quality issues, retrying"
                );
            }
            let portfolio_decision =
                portfolio_decision.expect("at least one LLM attempt must succeed");
            result.agent_state.sender = "Portfolio Manager".to_string();
            result.agent_state.final_trade_decision = portfolio_decision.rendered_decision();
            result.agent_state.structured_portfolio_decision =
                crate::models::StructuredPortfolioDecision {
                    rating: crate::models::Rating::parse(&portfolio_decision.rating),
                    raw_rating: portfolio_decision.rating.clone(),
                    calibrated_rating: portfolio_decision.rating.clone(),
                    confidence: portfolio_decision.confidence_string().into(),
                    risk_assessment: portfolio_decision.risk_assessment.clone().into(),
                    executive_summary: portfolio_decision.executive_summary.clone().into(),
                    investment_thesis: portfolio_decision.investment_thesis.clone().into(),
                    rationale: portfolio_decision.rationale.clone().into(),
                    price_target: portfolio_decision
                        .price_target
                        .as_ref()
                        .map(crate::engine::llm::parse::normalize_value)
                        .unwrap_or_default(),
                    confirmation_level: portfolio_decision
                        .confirmation_level
                        .as_ref()
                        .map(crate::engine::llm::parse::normalize_value)
                        .unwrap_or_default(),
                    invalidation_level: portfolio_decision
                        .invalidation_level
                        .as_ref()
                        .map(crate::engine::llm::parse::normalize_value)
                        .unwrap_or_default(),
                    target_type: String::new(),
                    target_reference: portfolio_decision
                        .target_reference
                        .clone()
                        .unwrap_or_default(),
                    target_condition: portfolio_decision
                        .target_condition
                        .clone()
                        .unwrap_or_default(),
                    time_horizon: portfolio_decision.time_horizon.clone().unwrap_or_default(),
                    missing_evidence_ladder: crate::models::MissingEvidenceLadder {
                        tolerable_gaps: portfolio_decision
                            .missing_evidence_ladder
                            .tolerable_gaps
                            .clone(),
                        manageable_gaps: portfolio_decision
                            .missing_evidence_ladder
                            .manageable_gaps
                            .clone(),
                        blocking_gaps: portfolio_decision
                            .missing_evidence_ladder
                            .blocking_gaps
                            .clone(),
                    },
                    trigger_checklist: portfolio_decision.trigger_checklist.clone(),
                    markdown: result.agent_state.final_trade_decision.clone(),
                };
            // Wire time-stop from portfolio decision if trader didn't provide it
            if result
                .agent_state
                .structured_trader_plan
                .time_stop_deadline
                .is_empty()
            {
                result.agent_state.structured_trader_plan.time_stop_deadline = portfolio_decision
                    .time_stop_deadline
                    .clone()
                    .unwrap_or_default();
            }
            if result
                .agent_state
                .structured_trader_plan
                .time_stop_reason
                .is_empty()
            {
                result.agent_state.structured_trader_plan.time_stop_reason = portfolio_decision
                    .time_stop_reason
                    .clone()
                    .unwrap_or_default();
            }
            result.graph.risk_debate.judge_decision =
                result.agent_state.final_trade_decision.clone();
            result.sync_derived_fields();
            crate::engine::analysis::graph::push_checkpoint(
                result,
                "portfolio_manager",
                "Portfolio Manager",
                "completed",
                portfolio_decision.rendered_decision(),
            );
            result.graph.reflection.status = "completed".to_string();
            result.graph.reflection.reflection = portfolio_decision.rendered_reflection();
            result.graph.reflection.source = "portfolio_decision_reflection".to_string();
            result.artifacts.llm_token_usage = deep_llm.usage_summary().await;
            self.persist_runtime_stage(result, "portfolio", "Portfolio Manager")
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_decision_context_empty() {
        assert_eq!(compact_decision_context("", 100), "");
    }

    #[test]
    fn compact_decision_context_zero_max() {
        assert_eq!(compact_decision_context("hello", 0), "");
    }

    #[test]
    fn compact_decision_context_short_text() {
        let text = "Short text";
        assert_eq!(compact_decision_context(text, 100), text);
    }

    #[test]
    fn compact_decision_context_truncates_long_text() {
        let lines: Vec<String> = (0..20)
            .map(|i| format!("Line {i} with some content"))
            .collect();
        let text = lines.join("\n");
        let result = compact_decision_context(&text, 100);
        assert!(result.chars().count() <= 100);
    }

    #[test]
    fn compact_decision_context_prioritizes_keywords() {
        let text = "Random line\nRecommend buying AAPL\nAnother random\nRisk is low\nMore text";
        let result = compact_decision_context(&text, 200);
        assert!(result.contains("Recommend"));
        assert!(result.contains("Risk"));
    }

    #[test]
    fn compact_decision_context_respects_max_chars() {
        let text = "Line one recommend\nLine two rating\nLine three confidence\nLine four risk\nLine five trigger\nLine six invalidation\nLine seven stop\nLine eight target\nLine nine entry\nLine ten price\nLine eleven support\nLine twelve resistance";
        let result = compact_decision_context(&text, 80);
        assert!(result.chars().count() <= 80);
    }

    #[test]
    fn compact_decision_context_single_long_line() {
        let text = "a".repeat(200);
        let result = compact_decision_context(&text, 50);
        assert!(result.chars().count() <= 50);
    }
}

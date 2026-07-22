use crate::AnalysisResult;
use crate::llm::parse::{DiagnosisIssue, IssueSeverity};
use crate::llm::types::HasConfidence;
use crate::task_manager::TaskRunParams;

use super::prepare::compact_decision_context;

impl crate::TaskManager {
    pub(crate) async fn run_research_manager_stage(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        quick_llm: &crate::llm::LlmClient,
        deep_llm: &crate::llm::LlmClient,
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
            let calibration_memo = crate::llm::LlmClient::calibration_memo(
                &params.memory_context,
                &params.market_type,
                &params.analysis_date,
            );
            self.refresh_structured_report_snapshot(result).await?;
            let fact_sheet = super::super::facts::build_decision_fact_sheet(result);
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
            let mut research_manager = None::<crate::llm::GeneratedResearchManager>;
            let mut last_issues = Vec::new();
            for retry in 0..=2u32 {
                let hint = if retry == 0 {
                    None
                } else {
                    Some(crate::llm::retry::default_retry_hint_builder(
                        &last_issues,
                        retry,
                    ))
                };
                let candidate = research_llm
                    .generate_research_manager(crate::llm::ResearchManagerParams {
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
            result.agent_state.structured_research_plan = crate::StructuredResearchPlan {
                recommendation: research_manager.recommendation.clone().into(),
                confidence: research_manager.confidence_string().into(),
                risk_assessment: research_manager.risk_assessment.clone().into(),
                rationale: research_manager.rationale.clone().into(),
                strategic_actions: research_manager.strategic_actions.clone().into(),
                missing_evidence_ladder: crate::MissingEvidenceLadder {
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
            crate::report::graph::push_checkpoint(
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
        quick_llm: &crate::llm::LlmClient,
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
            let calibration_memo = crate::llm::LlmClient::calibration_memo(
                &params.memory_context,
                &params.market_type,
                &params.analysis_date,
            );
            self.refresh_structured_report_snapshot(result).await?;
            let fact_sheet = super::super::facts::build_decision_fact_sheet(result);
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
            let mut trader = None::<crate::llm::GeneratedTraderDecision>;
            let mut last_issues = Vec::new();
            for retry in 0..=2u32 {
                let hint = if retry == 0 {
                    None
                } else {
                    Some(crate::llm::retry::default_retry_hint_builder(
                        &last_issues,
                        retry,
                    ))
                };
                let candidate = quick_llm
                    .generate_trader_decision(crate::llm::TraderDecisionParams {
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
                    // Check required fields for directional actions
                    let is_directional = matches!(candidate.action.trim(), "Buy" | "Sell");
                    if is_directional {
                        let entry_empty = candidate
                            .entry_price
                            .as_ref()
                            .map(crate::llm::parse::normalize_value)
                            .unwrap_or_default()
                            .trim()
                            .is_empty();
                        let stop_empty = candidate
                            .stop_loss
                            .as_ref()
                            .map(crate::llm::parse::normalize_value)
                            .unwrap_or_default()
                            .trim()
                            .is_empty();
                        let horizon_empty = candidate
                            .time_horizon
                            .as_deref()
                            .unwrap_or("")
                            .trim()
                            .is_empty();
                        if entry_empty {
                            v.push(DiagnosisIssue::error(
                                "trader_decision",
                                "entry_price",
                                "entry_price is required for Buy/Sell but was empty",
                            ));
                        }
                        if stop_empty {
                            v.push(DiagnosisIssue::error(
                                "trader_decision",
                                "stop_loss",
                                "stop_loss is required for Buy/Sell but was empty",
                            ));
                        }
                        if horizon_empty {
                            v.push(DiagnosisIssue::error(
                                "trader_decision",
                                "time_horizon",
                                "time_horizon is required for Buy/Sell but was empty",
                            ));
                        }
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
            result.agent_state.structured_trader_plan = crate::StructuredTraderPlan {
                action: trader.action.clone().into(),
                raw_action: trader.action.clone(),
                calibrated_action: trader.action.clone(),
                reasoning: trader.reasoning.clone().into(),
                entry_price: trader
                    .entry_price
                    .as_ref()
                    .map(crate::llm::parse::normalize_value)
                    .unwrap_or_default(),
                stop_loss: trader
                    .stop_loss
                    .as_ref()
                    .map(crate::llm::parse::normalize_value)
                    .unwrap_or_default(),
                confirmation_level: trader
                    .confirmation_level
                    .as_ref()
                    .map(crate::llm::parse::normalize_value)
                    .unwrap_or_default(),
                target_reference: trader.target_reference.clone().unwrap_or_default(),
                target_condition: trader.target_condition.clone().unwrap_or_default(),
                time_horizon: trader.time_horizon.clone().unwrap_or_default(),
                position_sizing: trader.position_sizing.clone().unwrap_or_default(),
                proposal: crate::LocalText::new(trader.action.trim().to_string()),
                execution_trigger_checklist: trader.execution_trigger_checklist.clone(),
                stop_execution_discipline: None,
                blocking_gaps: trader.blocking_gaps.clone(),
                time_stop_deadline: trader.time_stop_deadline.clone().unwrap_or_default(),
                time_stop_reason: trader.time_stop_reason.clone().unwrap_or_default(),
                markdown: result.agent_state.trader_investment_plan.clone(),
            };
            result.sync_derived_fields();
            result.artifacts.llm_token_usage = quick_llm.usage_summary().await;
            crate::report::graph::push_checkpoint(
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
        deep_llm: &crate::llm::LlmClient,
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
            let calibration_memo = crate::llm::LlmClient::calibration_memo(
                &params.memory_context,
                &params.market_type,
                &params.analysis_date,
            );
            self.refresh_structured_report_snapshot(result).await?;
            let fact_sheet = super::super::facts::build_decision_fact_sheet(result);
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
            let mut portfolio_decision = None::<crate::llm::GeneratedPortfolioDecision>;
            let mut last_issues = Vec::new();
            for retry in 0..=2u32 {
                let hint = if retry == 0 {
                    None
                } else {
                    Some(crate::llm::retry::default_retry_hint_builder(
                        &last_issues,
                        retry,
                    ))
                };
                let candidate = deep_llm
                    .generate_portfolio_decision(crate::llm::PortfolioDecisionParams {
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
                    // Check required fields for directional ratings
                    let is_directional = matches!(
                        candidate.rating.trim(),
                        "Buy" | "Overweight" | "Underweight" | "Sell"
                    );
                    if is_directional {
                        let price_target_empty = candidate
                            .price_target
                            .as_ref()
                            .map(crate::llm::parse::normalize_value)
                            .unwrap_or_default()
                            .trim()
                            .is_empty();
                        let horizon_empty = candidate
                            .time_horizon
                            .as_deref()
                            .unwrap_or("")
                            .trim()
                            .is_empty();
                        let confirmation_empty = candidate
                            .confirmation_level
                            .as_ref()
                            .map(crate::llm::parse::normalize_value)
                            .unwrap_or_default()
                            .trim()
                            .is_empty();
                        let invalidation_empty = candidate
                            .invalidation_level
                            .as_ref()
                            .map(crate::llm::parse::normalize_value)
                            .unwrap_or_default()
                            .trim()
                            .is_empty();
                        if price_target_empty {
                            v.push(DiagnosisIssue::error(
                                "portfolio_decision",
                                "price_target",
                                "price_target is required for directional rating but was empty",
                            ));
                        }
                        if horizon_empty {
                            v.push(DiagnosisIssue::error(
                                "portfolio_decision",
                                "time_horizon",
                                "time_horizon is required for directional rating but was empty",
                            ));
                        }
                        if confirmation_empty {
                            v.push(DiagnosisIssue::error(
                                "portfolio_decision", "confirmation_level",
                                "confirmation_level is required for directional rating but was empty",
                            ));
                        }
                        if invalidation_empty {
                            v.push(DiagnosisIssue::error(
                                "portfolio_decision", "invalidation_level",
                                "invalidation_level is required for directional rating but was empty",
                            ));
                        }
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
            result.agent_state.structured_portfolio_decision = crate::StructuredPortfolioDecision {
                rating: crate::Rating::parse(&portfolio_decision.rating),
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
                    .map(crate::llm::parse::normalize_value)
                    .unwrap_or_default(),
                confirmation_level: portfolio_decision
                    .confirmation_level
                    .as_ref()
                    .map(crate::llm::parse::normalize_value)
                    .unwrap_or_default(),
                invalidation_level: portfolio_decision
                    .invalidation_level
                    .as_ref()
                    .map(crate::llm::parse::normalize_value)
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
                missing_evidence_ladder: crate::MissingEvidenceLadder {
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
            crate::report::graph::push_checkpoint(
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

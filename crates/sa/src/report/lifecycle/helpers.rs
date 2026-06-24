use crate::data::{FundamentalsSnapshot, NewsItem, QuoteSnapshot};
use crate::{
    AnalysisOutcomeRequest, AnalysisResult, AnalysisStep, ResultStage, StepStatus, TaskStatus,
};
use crate::{TaskManager, TaskRunParams};

impl TaskManager {
    pub(super) async fn resolve_pending_entries(
        &self,
        ticker: &str,
        params: &TaskRunParams,
    ) -> anyhow::Result<()> {
        let pending = self
            .memory_log
            .load_entries()
            .await?
            .into_iter()
            .filter(|entry| entry.pending && entry.ticker.eq_ignore_ascii_case(ticker))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return Ok(());
        }

        let mut updates = Vec::new();
        for entry in pending {
            let Some(outcome_return) = self
                .market_data
                .fetch_return_since(&entry.ticker, &entry.trade_date, 5)
                .await?
            else {
                continue;
            };
            let benchmark_return = self
                .market_data
                .fetch_return_since("SPY", &entry.trade_date, 5)
                .await?
                .unwrap_or(0.0);
            let llm = self
                .resolve_llm_client(&TaskRunParams::for_reflection_with_llm(
                    entry.trade_date.clone(),
                    &params.language,
                    params,
                ))
                .await?;
            let risk_text = format!(
                "Actual return {:.4}; benchmark return {:.4}",
                outcome_return, benchmark_return
            );
            let reflection = llm
                .generate_reflection(crate::llm::ReflectionParams {
                    symbol: &entry.ticker,
                    market_type: "unknown",
                    analysis_date: &entry.trade_date,
                    summary: &entry.final_trade_decision,
                    recommendation: &entry.rating,
                    rationale: &entry.final_trade_decision,
                    risk_assessment: &risk_text,
                })
                .await?;
            updates.push(crate::memory::MemoryOutcomeUpdate {
                ticker: entry.ticker,
                trade_date: entry.trade_date,
                outcome_return,
                benchmark_return,
                holding_days: 5,
                reflection,
            });
        }

        self.memory_log
            .batch_update_with_outcomes_async(&updates)
            .await?;
        Ok(())
    }

    pub(crate) async fn initial_memory_context(
        &self,
        ticker: &str,
        market: &str,
        quote: Option<&QuoteSnapshot>,
        fundamentals: Option<&FundamentalsSnapshot>,
        news_items: &[NewsItem],
    ) -> crate::memory::MemoryContextBundleWithTags {
        let setup_tags =
            Self::derive_pre_analysis_setup_tags(ticker, market, quote, fundamentals, news_items);
        let query = crate::memory::MemoryQuery {
            ticker: ticker.to_string(),
            market: market.to_string(),
            setup_tags: setup_tags.clone(),
            user_id: String::new(),
        };
        let memory_context = self
            .memory_log
            .past_context_bundle_async_with_query(&query, 5, 3)
            .await
            .unwrap_or_default();
        let setup_match_stats = self
            .memory_log
            .effective_setup_match_stats(&query)
            .await
            .unwrap_or_default();
        crate::memory::MemoryContextBundleWithTags {
            context_text: memory_context.context_text,
            source: memory_context.source,
            retrieval_mode: memory_context.retrieval_mode,
            embedding_provider: memory_context.embedding_provider,
            embedding_failure_reason: memory_context.embedding_failure_reason,
            same_ticker_count: memory_context.same_ticker_count,
            cross_ticker_count: memory_context.cross_ticker_count,
            vector_hit_count: memory_context.vector_hit_count,
            effective_top_k: memory_context.effective_top_k,
            same_ticker_highlights: memory_context.same_ticker_highlights,
            cross_ticker_highlights: memory_context.cross_ticker_highlights,
            setup_tags,
            used_setup_filtered_retrieval: !query.setup_tags.is_empty(),
            used_setup_fallback_calibration: setup_match_stats.used_fallback,
            setup_calibration_sample_count: setup_match_stats.calibration_sample_count,
            setup_match_count: setup_match_stats.total_match_count,
            setup_pending_match_count: setup_match_stats.pending_match_count,
            setup_resolved_match_count: setup_match_stats.resolved_match_count,
            setup_match_hit_rate: setup_match_stats.hit_rate,
            setup_match_avg_alpha_return: setup_match_stats.avg_alpha_return,
            setup_long_match_count: setup_match_stats.long_match_count,
            setup_short_match_count: setup_match_stats.short_match_count,
            setup_neutral_match_count: setup_match_stats.neutral_match_count,
        }
    }

    fn derive_pre_analysis_setup_tags(
        _ticker: &str,
        _market: &str,
        quote: Option<&QuoteSnapshot>,
        fundamentals: Option<&FundamentalsSnapshot>,
        news_items: &[NewsItem],
    ) -> Vec<String> {
        let mut tags = Vec::new();

        if let Some(quote) = quote {
            if quote.close > 0.0 {
                let hundred = 100.0;
                let intraday_change = ((quote.close - quote.open) / quote.close.abs()) * hundred;
                let intraday_range = ((quote.high - quote.low) / quote.close.abs()) * hundred;
                if intraday_change >= 2.0 || (quote.close > quote.open && intraday_range >= 3.5) {
                    tags.push("trend_confirmed".to_string());
                }
                if intraday_range >= 4.5 {
                    tags.push("event_driven".to_string());
                }
            }
        }

        if let Some(fundamentals) = fundamentals {
            let market_cap = fundamentals.market_cap.unwrap_or_default();
            let net_income = fundamentals.net_income_usd.unwrap_or_default();
            let revenue = fundamentals.revenues_usd.unwrap_or_default();
            let free_cash_flow = fundamentals.free_cash_flow_usd.unwrap_or_default();
            let billion = 1_000_000_000.0;
            let has_quality_scale =
                market_cap > 0.0 && (net_income > 0.0 || free_cash_flow > 0.0 || revenue > billion);
            if has_quality_scale {
                tags.push("fundamental_quality".to_string());
            }
            if market_cap > 0.0 && free_cash_flow > 0.0 && (free_cash_flow / market_cap) < 0.03 {
                tags.push("valuation_sensitive".to_string());
            }
        }

        if !news_items.is_empty() {
            tags.push("event_driven".to_string());
        }

        if !tags.iter().any(|tag| tag == "watchlist_only") {
            tags.push("watchlist_only".to_string());
        }

        let mut ordered = Vec::new();
        for tag in tags {
            if !ordered.iter().any(|existing| existing == &tag) {
                ordered.push(tag);
            }
        }
        ordered
    }

    pub(super) fn infer_result_stage(result: &AnalysisResult) -> ResultStage {
        if !result.agent_state.final_trade_decision.trim().is_empty() {
            ResultStage::Complete
        } else if !result.graph.risk_debate.history.trim().is_empty() {
            ResultStage::Risk
        } else if !result.agent_state.trader_investment_plan.trim().is_empty() {
            ResultStage::Trader
        } else if !result.agent_state.investment_plan.trim().is_empty() {
            ResultStage::Research
        } else if !result.graph.investment_debate.history.trim().is_empty() {
            ResultStage::Debate
        } else if !result.graph.analysts.is_empty() {
            ResultStage::Analysts
        } else {
            ResultStage::Overview
        }
    }

    pub(super) fn steps_for_progress(
        progress: i32,
        status: &TaskStatus,
        current_step_name: &str,
        result_stage: Option<&ResultStage>,
        report_stage_state: Option<&crate::ReportStageState>,
    ) -> Vec<AnalysisStep> {
        crate::TASK_STEPS
            .into_iter()
            .enumerate()
            .map(|(index, (name, description, _gate))| {
                let status = match status {
                    TaskStatus::Completed => StepStatus::Success,
                    TaskStatus::Cancelled => {
                        if current_step_name == name {
                            StepStatus::Error
                        } else if Self::step_completed(
                            index,
                            progress,
                            result_stage,
                            report_stage_state,
                        ) {
                            StepStatus::Success
                        } else {
                            StepStatus::Pending
                        }
                    }
                    TaskStatus::Failed => {
                        if current_step_name == name {
                            StepStatus::Error
                        } else if Self::step_completed(
                            index,
                            progress,
                            result_stage,
                            report_stage_state,
                        ) {
                            StepStatus::Success
                        } else {
                            StepStatus::Pending
                        }
                    }
                    TaskStatus::Running => {
                        if current_step_name == name {
                            StepStatus::Active
                        } else if Self::step_completed(
                            index,
                            progress,
                            result_stage,
                            report_stage_state,
                        ) {
                            StepStatus::Success
                        } else {
                            StepStatus::Pending
                        }
                    }
                    TaskStatus::Pending => StepStatus::Pending,
                };
                AnalysisStep {
                    name: name.to_string(),
                    description: description.to_string(),
                    status,
                }
            })
            .collect()
    }

    fn step_completed(
        index: usize,
        progress: i32,
        result_stage: Option<&ResultStage>,
        report_stage_state: Option<&crate::ReportStageState>,
    ) -> bool {
        match index {
            0 => {
                report_stage_state.is_some_and(|state| {
                    state.market || state.sentiment || state.news || state.fundamentals
                }) || progress >= 15
            }
            1 => report_stage_state.is_some_and(|state| state.market && state.fundamentals),
            2 => report_stage_state.is_some_and(|state| state.news && state.sentiment),
            3 => {
                matches!(
                    result_stage,
                    Some(ResultStage::Research)
                        | Some(ResultStage::Trader)
                        | Some(ResultStage::Risk)
                        | Some(ResultStage::Portfolio)
                        | Some(ResultStage::Complete)
                ) || report_stage_state
                    .is_some_and(|state| state.research_plan || state.trader_plan)
            }
            4 => {
                matches!(
                    result_stage,
                    Some(ResultStage::Portfolio) | Some(ResultStage::Complete)
                ) || report_stage_state.is_some_and(|state| state.portfolio_decision)
            }
            _ => progress >= 100,
        }
    }
}
impl TaskManager {
    pub(super) fn strip_incomplete_result_payload(result: &mut AnalysisResult) {
        let stage = result.report_stage();

        if !stage.market {
            result.agent_state.market_report.clear();
        }
        if !stage.fundamentals {
            result.agent_state.fundamentals_report.clear();
        }
        if !stage.news {
            result.agent_state.news_report.clear();
        }
        if !stage.sentiment {
            result.agent_state.sentiment_report.clear();
        }
        if !stage.bull_research {
            result.graph.investment_debate.bull_history.clear();
        }
        if !stage.bear_research {
            result.graph.investment_debate.bear_history.clear();
        }
        if !stage.research_plan {
            result.agent_state.investment_plan.clear();
            result.agent_state.structured_research_plan = Default::default();
        }
        if !stage.trader_plan {
            result.agent_state.trader_investment_plan.clear();
            result.agent_state.structured_trader_plan = Default::default();
        }
        if !stage.risk_debate {
            result.agent_state.risk_debate_state = Default::default();
        }
        if !stage.portfolio_decision {
            result.agent_state.final_trade_decision.clear();
            result.agent_state.structured_portfolio_decision = Default::default();
            result.report = Default::default();
        }
        if !stage.reflection {
            result.graph.reflection = Default::default();
        }
    }

    pub async fn auto_reflect_outcomes(&self, holding_days: usize) -> anyhow::Result<usize> {
        let entries = self.memory_log.load_entries().await?;
        let mut updated = 0usize;
        for entry in entries {
            if !entry.pending {
                continue;
            }
            let Some(outcome_return) = self
                .market_data
                .fetch_return_since(&entry.ticker, &entry.trade_date, holding_days)
                .await?
            else {
                continue;
            };
            let benchmark_return = self
                .market_data
                .fetch_return_since("SPY", &entry.trade_date, holding_days)
                .await?
                .unwrap_or(0.0);
            let llm = self
                .resolve_llm_client(&TaskRunParams::for_reflection(
                    entry.trade_date.clone(),
                    "zh-CN",
                ))
                .await?;
            let risk_text = format!(
                "Actual return {:.4}; benchmark return {:.4}",
                outcome_return, benchmark_return
            );
            let reflection = llm
                .generate_reflection(crate::llm::ReflectionParams {
                    symbol: &entry.ticker,
                    market_type: "unknown",
                    analysis_date: &entry.trade_date,
                    summary: &entry.final_trade_decision,
                    recommendation: &entry.rating,
                    rationale: &entry.final_trade_decision,
                    risk_assessment: &risk_text,
                })
                .await?;
            self.memory_log
                .update_outcome_async(
                    &entry.ticker,
                    &entry.trade_date,
                    outcome_return,
                    benchmark_return,
                    reflection,
                )
                .await?;
            updated += 1;
        }
        Ok(updated)
    }

    pub async fn record_outcome_reflection(
        &self,
        request: AnalysisOutcomeRequest,
    ) -> anyhow::Result<()> {
        let llm = self
            .resolve_llm_client(&TaskRunParams::for_reflection(
                request.trade_date.clone(),
                "zh-CN",
            ))
            .await?;
        let summary_text = format!(
            "Outcome return: {:.4}, benchmark return: {:.4}",
            request.outcome_return, request.benchmark_return
        );
        let reflection = llm
            .generate_reflection(crate::llm::ReflectionParams {
                symbol: &request.ticker,
                market_type: "unknown",
                analysis_date: &request.trade_date,
                summary: &summary_text,
                recommendation: "Outcome Review",
                rationale: "Review strategy by comparing actual vs benchmark returns.",
                risk_assessment: "Watch for deviation and assumption failure.",
            })
            .await?;
        self.memory_log
            .update_outcome_async(
                &request.ticker,
                &request.trade_date,
                request.outcome_return,
                request.benchmark_return,
                reflection,
            )
            .await?;
        Ok(())
    }

    pub async fn evaluation_summary(&self) -> anyhow::Result<serde_json::Value> {
        self.memory_log.evaluation_summary().await
    }
}

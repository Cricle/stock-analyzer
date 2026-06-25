impl AnalysisResult {
    fn inferred_stock_name_from_runtime(&self) -> Option<String> {
        let state = self.analyst_runtime_state("fundamentals")?;
        for observation in &state.tool_history {
            if !observation.success || observation.tool_name != "get_fundamentals" {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&observation.output) else {
                continue;
            };
            let company_name = value
                .get("company_name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            return Some(company_name.to_string());
        }
        None
    }

    pub(crate) fn structured_research_plan(&self) -> StructuredResearchPlan {
        self.agent_state.structured_research_plan.clone()
    }

    pub(crate) fn structured_trader_plan(&self) -> StructuredTraderPlan {
        self.agent_state.structured_trader_plan.clone()
    }

    pub(crate) fn structured_portfolio_decision(&self) -> StructuredPortfolioDecision {
        self.agent_state.structured_portfolio_decision.clone()
    }

    pub fn derived_summary(&self) -> String {
        let portfolio_decision = self.structured_portfolio_decision();
        if !portfolio_decision.executive_summary.trim().is_empty() {
            portfolio_decision.executive_summary.to_string()
        } else {
            let research_plan = self.structured_research_plan();
            if !research_plan.rationale.trim().is_empty() {
                research_plan.rationale.to_string()
            } else {
                format!(
                    "{} {} 初始状态已建立，等待分析师链路生成研究产物。",
                    self.symbol, self.analysis_date
                )
            }
        }
    }

    pub fn derived_recommendation(&self) -> String {
        let portfolio_decision = self.structured_portfolio_decision();
        if portfolio_decision.rating != Rating::Hold || !portfolio_decision.raw_rating.trim().is_empty() {
            portfolio_decision.rating.to_string()
        } else {
            let research_plan = self.structured_research_plan();
            if !research_plan.recommendation.trim().is_empty() {
                research_plan.recommendation.to_string()
            } else {
                "Hold".to_string()
            }
        }
    }

    pub fn derived_risk_assessment(&self) -> String {
        let portfolio_decision = self.structured_portfolio_decision();
        if !portfolio_decision.risk_assessment.trim().is_empty() {
            portfolio_decision.risk_assessment.to_string()
        } else {
            let research_plan = self.structured_research_plan();
            if !research_plan.risk_assessment.trim().is_empty() {
                research_plan.risk_assessment.to_string()
            } else {
                "待分析".to_string()
            }
        }
    }

    pub fn derived_confidence(&self) -> String {
        let portfolio_decision = self.structured_portfolio_decision();
        if !portfolio_decision.confidence.trim().is_empty() {
            portfolio_decision.confidence.to_string()
        } else {
            let research_plan = self.structured_research_plan();
            research_plan.confidence.to_string()
        }
    }

    pub fn derived_rationale(&self) -> String {
        let portfolio_decision = self.structured_portfolio_decision();
        if !portfolio_decision.investment_thesis.trim().is_empty() {
            portfolio_decision.investment_thesis.to_string()
        } else {
            let research_plan = self.structured_research_plan();
            research_plan.rationale.to_string()
        }
    }

    pub fn sync_derived_fields(&mut self) {
        self.agent_state.investment_debate_state = self.graph.investment_debate.clone();
        self.agent_state.risk_debate_state = self.graph.risk_debate.clone();
        let current_name = self.stock_name.trim();
        if (current_name.is_empty() || current_name.eq_ignore_ascii_case(self.symbol.trim()))
            && let Some(company_name) = self.inferred_stock_name_from_runtime()
        {
            self.stock_name = company_name;
        }
    }

    pub fn rebuild_report(
        &mut self,
        calibration_profile: &crate::scoring::CalibrationProfile,
    ) {
        self.report = StructuredReport::from_result(self, calibration_profile);
        self.ic_report = StructuredReport::ic_chair_from_report(self, &self.report);
    }

    pub fn apply_calibrated_markdown(&mut self) {
        // When blocking gaps exist, override position_sizing before rendering markdown
        // so the execution plan output doesn't contain stale LLM sizing like "2%"
        // that contradicts the "0% blocker" discipline enforced elsewhere.
        let has_blockers = !self.report.trader_plan.blocking_gaps.is_empty()
            || !self.report.portfolio_decision.missing_evidence_ladder.blocking_gaps.is_empty();
        if has_blockers {
            self.report.trader_plan.position_sizing =
                "0%——关键证据尚未补齐，不新增方向性暴露".to_string();
        }
        self.agent_state.structured_trader_plan = self.report.trader_plan.clone();
        self.agent_state.structured_portfolio_decision = self.report.portfolio_decision.clone();
        self.agent_state.trader_investment_plan = self.report.trader_plan.render_markdown();
        let calibration_appendix = crate::analysis::render_calibration_discipline_markdown(
            &self.report,
            &self.artifacts.memory_context,
            &self.artifacts.calibration_memo,
        );
        self.agent_state.final_trade_decision = format!(
            "{}\n\n{}",
            self.report.portfolio_decision.render_markdown(),
            calibration_appendix
        );
        self.sync_derived_fields();

        self.report.trader_plan.markdown = self.agent_state.trader_investment_plan.clone();
        self.report.portfolio_decision.markdown = self.agent_state.final_trade_decision.clone();
        self.agent_state.structured_trader_plan.markdown =
            self.agent_state.trader_investment_plan.clone();
        self.agent_state.structured_portfolio_decision.markdown =
            self.agent_state.final_trade_decision.clone();
        for section in &mut self.report.sections {
            match section.key.as_str() {
                "trader_plan" => {
                    section.content = self.agent_state.trader_investment_plan.trim().to_string();
                }
                "portfolio_decision" => {
                    section.content = self.agent_state.final_trade_decision.trim().to_string();
                }
                _ => {}
            }
        }
    }

    pub fn report_stage(&self) -> ReportStageState {
        ReportStageState {
            overview: true,
            market: !self.agent_state.market_report.trim().is_empty(),
            fundamentals: !self.agent_state.fundamentals_report.trim().is_empty(),
            news: !self.agent_state.news_report.trim().is_empty(),
            sentiment: !self.agent_state.sentiment_report.trim().is_empty(),
            bull_research: !self.graph.investment_debate.bull_history.trim().is_empty(),
            bear_research: !self.graph.investment_debate.bear_history.trim().is_empty(),
            research_plan: !self.agent_state.investment_plan.trim().is_empty(),
            trader_plan: !self.agent_state.trader_investment_plan.trim().is_empty(),
            risk_debate: !self.agent_state.risk_debate_state.history.trim().is_empty(),
            portfolio_decision: !self.agent_state.final_trade_decision.trim().is_empty(),
            reflection: !self.graph.reflection.reflection.trim().is_empty(),
        }
    }
}

/// Label for reward-risk ratio in Chinese.
pub fn rr_label(rr: f64) -> &'static str {
    if rr >= 2.0 {
        "赔率充裕"
    } else if rr >= 1.2 {
        "赔率尚可"
    } else if rr >= 0.5 {
        "赔率偏弱"
    } else {
        "赔率极差"
    }
}

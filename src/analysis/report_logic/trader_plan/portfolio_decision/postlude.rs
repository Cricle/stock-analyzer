
impl StructuredPortfolioDecision {
    fn raw_directional_anchor(&self) -> Option<&str> {
        let raw = self.raw_rating.trim();
        (!raw.is_empty()).then_some(raw)
    }

    fn has_authoritative_override(&self, trader_plan: &StructuredTraderPlan) -> bool {
        let rating_changed = !self.raw_rating.trim().is_empty()
            && Rating::parse(&self.raw_rating) != self.rating;
        let action_changed = !trader_plan.raw_action.trim().is_empty()
            && !trader_plan
                .raw_action
                .trim()
                .eq_ignore_ascii_case(trader_plan.action.trim());
        rating_changed || action_changed
    }

    /// Compute Authoritative_summary.
    pub fn authoritative_summary(
        &self,
        trader_plan: &StructuredTraderPlan,
        confidence_score: i32,
        core_research_call: &CoreResearchCall,
        decision_view: &DecisionView,
    ) -> String {
        let rating = fallback_rating(self);
        let action = if trader_plan.action.trim().is_empty() {
            "Hold"
        } else {
            trader_plan.action.trim()
        };
        let has_override = self.has_authoritative_override(trader_plan);
        let thesis = if has_override {
            None
        } else {
            first_non_empty_sentence(&[
                self.investment_thesis.as_str(),
                self.rationale.as_str(),
                self.executive_summary.as_str(),
            ])
        };
        let risk =
            first_non_empty_sentence(&[humanize_risk_assessment(self.risk_assessment.as_str()).as_str()]);
        let confirmation = visible_confirmation_reference(self);
        let invalidation =
            (!self.invalidation_level.trim().is_empty()).then(|| self.invalidation_level.trim().to_string());
        let target = visible_target_reference(self);
        let raw_anchor = self.raw_directional_anchor();
        let conditional_bullish = (rating.is_bullish()
            || raw_anchor.is_some_and(|value| Rating::parse(value).is_bullish()))
            && !confirmation.as_deref().unwrap_or_default().is_empty()
            && action.eq_ignore_ascii_case("Hold");
        let conditional_bearish = raw_anchor.is_some_and(|value| Rating::parse(value).is_bearish())
            && !confirmation.as_deref().unwrap_or_default().is_empty()
            && action.eq_ignore_ascii_case("Hold");
        let action_label = describe_decision_action(&decision_view.action);
        let execution_state = describe_execution_state(&decision_view.execution_state);
        let research_call = describe_core_research_call(core_research_call);
        let mut parts = vec![format!(
            "当前核心研究结论为 {research_call}，执行状态是{execution_state}，当前可执行把握为 {confidence_score}/100，下一步动作是{action_label}。当前更重要的不是重复证明长期逻辑，而是确认市场是否会把主路径真正走出来。"
        )];
        if conditional_bullish || matches!(core_research_call, CoreResearchCall::BuyOnConfirmation) {
            parts[0] = if matches!(decision_view.action, DecisionAction::ProbePosition) {
                format!(
                    "当前核心研究结论仍是条件确认后偏多，执行状态仍是条件待确认。当前可执行把握为 {confidence_score}/100，下一步动作是{action_label}，重点是先围绕确认线小仓试探并严格控风险，而不是直接追价或放大暴露。"
                )
            } else {
                format!(
                    "当前核心研究结论仍是条件确认后偏多，执行状态仍是条件待确认。当前可执行把握为 {confidence_score}/100，下一步动作是{action_label}，重点是等市场把主路径走成，而不是提前追价。"
                )
            };
        } else if conditional_bearish || matches!(core_research_call, CoreResearchCall::SellOnBreak) {
            parts[0] = format!(
                "当前核心研究结论已转为破位转空，执行状态仍是条件待确认。当前可执行把握为 {confidence_score}/100，下一步动作是{action_label}，重点是等待破位或风险证据真正完成，而不是过早放大防守动作。"
            );
        }
        parts.push(format!(
            "当前组合执行评级仍为 {rating}，这反映的是仓位纪律，而不是方向回到中性。"
        ));
        if has_override {
            parts.push(
                "原始组合经理结论更激进，但在证据尚未完成闭环前，最终输出选择了更强的执行纪律。"
                    .to_string(),
            );
        }
        if let Some(level) = confirmation
            .as_ref()
            .filter(|level| is_publishable_summary_reference(level))
        {
            parts.push(format!(
                "当前最值得盯住的确认位在 {}。",
                normalize_reference_phrase(level)
            ));
        }
        if let Some(level) = invalidation
            .as_ref()
            .map(|item| normalize_level_phrase(item))
            .filter(|level| is_publishable_summary_reference(level))
        {
            parts.push(format!("若出现 {level}，当前主张需要下修。"));
        }
        if let Some(target) = target
            .as_ref()
            .filter(|target| is_publishable_summary_reference(target))
        {
            parts.push(format!(
                "目标参考先看 {}。",
                normalize_reference_phrase(target)
            ));
        }
        if let Some(thesis) = thesis.as_ref() {
            parts.push(format!("核心判断：{thesis}"));
        }
        if let Some(risk) =
            risk.filter(|item| !is_semantically_similar(Some(item), thesis.as_ref()))
        {
            parts.push(format!("主要风险：{risk}"));
        }
        parts.join(" ")
    }

    fn authoritative_investment_thesis(
        &self,
        trader_plan: &StructuredTraderPlan,
        confidence_score: i32,
    ) -> String {
        let rating = fallback_rating(self);
        let action = if trader_plan.action.trim().is_empty() {
            "Hold"
        } else {
            trader_plan.action.trim()
        };
        let confirmation = visible_confirmation_reference(self);
        let raw_anchor = self.raw_directional_anchor();
        let support = if self.has_authoritative_override(trader_plan) {
            if raw_anchor.is_some_and(|value| Rating::parse(value).is_bullish()) {
                "方向层面并没有转空：研究证据仍然偏正面，但当前更适合等待更清晰的确认，而不是直接扩大风险暴露。"
                    .to_string()
            } else if raw_anchor.is_some_and(|value| Rating::parse(value).is_bearish()) {
                "方向层面并没有重新转多：研究证据仍然偏防守，但当前更适合等待更清晰的破位或风险确认。"
                    .to_string()
            } else {
                "方向证据仍然偏正面，但当前更适合等待更清晰的确认，而不是直接扩大风险暴露。"
                    .to_string()
            }
        } else {
            first_non_empty_sentence(&[
                self.investment_thesis.as_str(),
                self.rationale.as_str(),
                self.executive_summary.as_str(),
            ])
            .unwrap_or_else(|| {
                "方向证据仍然偏正面，但当前更适合等待更清晰的确认，而不是直接扩大风险暴露。"
                    .to_string()
            })
        };
        let support = strip_redundant_prefix(
            &support,
            &[
                "方向层面并没有转空：",
                "方向层面并没有重新转多：",
                &format!("Final stance: {rating}. Execution action: {action}."),
                &format!(
                    "The calibrated portfolio stance stays at {rating} with execution set to {action}."
                ),
            ],
        );
        let opening = if raw_anchor.is_some_and(|value| Rating::parse(value).is_bullish()) {
            format!(
                "方向层面并没有转空：当前组合结论维持 {rating}，执行动作保持 {action}，并不代表资产质量转弱，而是说明在当前 {confidence_score}/100 的可执行把握下，证据更支持先选定主路径，再等待市场给出足够清晰的完成式确认。"
            )
        } else if raw_anchor.is_some_and(|value| Rating::parse(value).is_bearish()) {
            format!(
                "方向层面并没有重新转多：当前组合结论维持 {rating}，执行动作保持 {action}，并不代表风险已经解除，而是说明在当前 {confidence_score}/100 的可执行把握下，证据更支持先守住防守框架，再等待更清晰的破位或风险确认。"
            )
        } else {
            format!(
                "当前维持 {rating}，执行动作保持 {action}，并不代表资产质量转弱，而是说明在当前 {confidence_score}/100 的可执行把握下，证据更支持先选定主路径，再等待市场给出足够清晰的完成式确认。"
            )
        };
        format!(
            "{opening} {} 支撑背景：{support}",
            confirmation
                .map(|level| format!("如果后续价格能有效处理 {level} 这一确认位，结论才有资格升级。"))
                .unwrap_or_else(|| "当前仍缺少一个足够可信的价格确认位，因此不宜把研究判断直接升级成仓位判断。".to_string())
        )
    }

    fn authoritative_rationale(
        &self,
        trader_plan: &StructuredTraderPlan,
        confidence_score: i32,
        calibration_reason: &str,
    ) -> String {
        let rating = fallback_rating(self);
        let action = if trader_plan.action.trim().is_empty() {
            "Hold"
        } else {
            trader_plan.action.trim()
        };
        let calibration_reason = strip_redundant_prefix(
            calibration_reason,
            &[
                &format!("最终建议收敛为 {rating}"),
                &format!("最终动作收敛为 {action}"),
            ],
        );
        format!(
            "本次维持 {rating}，并把执行动作留在 {action}，重点不是回避判断，而是承认当前 {confidence_score}/100 的可执行把握还不足以支持更激进的动作。更合理的做法仍是明确当前主路径，并等待更清晰的完成式确认。{calibration_reason}"
        )
    }

    /// Compute Render_markdown.
    pub fn render_markdown(&self) -> String {
        let risk_summary = humanize_risk_assessment(self.risk_assessment.as_str());
        let risk_sections = render_risk_assessment_sections(self.risk_assessment.as_str());
        let mut parts = vec![
            "# Portfolio Manager Decision".to_string(),
            String::new(),
            "## Final Rating".to_string(),
            format!("**Rating**: {}", self.rating),
            format!("**Confidence**: {}", self.confidence),
            format!("**Risk Assessment**: {}", risk_summary),
            String::new(),
            "## Executive Summary".to_string(),
            format!("**Executive Summary**: {}", self.executive_summary),
            String::new(),
            "## Investment Thesis".to_string(),
            format!("**Investment Thesis**: {}", self.investment_thesis),
        ];
        if !risk_sections.trim().is_empty() {
            parts.extend([
                String::new(),
                "## Risk Context".to_string(),
                risk_sections,
            ]);
        }
        if !self.rationale.trim().is_empty() {
            parts.extend([
                String::new(),
                "## Why This Call Won".to_string(),
                format!("**Rationale**: {}", self.rationale),
            ]);
        }
        if !self.confirmation_level.trim().is_empty() {
            parts.extend([
                String::new(),
                "## Confirmation Level".to_string(),
                format!("**Confirmation Level**: {}", self.confirmation_level),
            ]);
        }
        if !self.invalidation_level.trim().is_empty() {
            parts.extend([
                String::new(),
                "## Invalidation Level".to_string(),
                format!("**Invalidation Level**: {}", self.invalidation_level),
            ]);
        }
        if !self.target_reference.trim().is_empty() {
            parts.extend([
                String::new(),
                "## Target Reference".to_string(),
                format!("**Target Reference**: {}", self.target_reference),
            ]);
        }
        if !self.target_condition.trim().is_empty() {
            parts.extend([
                String::new(),
                "## Target Condition".to_string(),
                format!("**Target Condition**: {}", self.target_condition),
            ]);
        }
        if !self.price_target.trim().is_empty() {
            parts.extend([
                String::new(),
                "## Price Target".to_string(),
                format!("**Price Target**: {}", self.price_target),
            ]);
        }
        if !self.time_horizon.trim().is_empty() {
            parts.extend([
                String::new(),
                "## Time Horizon".to_string(),
                format!("**Time Horizon**: {}", self.time_horizon),
            ]);
        }
        parts.join("\n")
    }
}


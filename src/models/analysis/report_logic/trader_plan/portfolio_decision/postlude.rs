
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

    fn authoritative_summary(
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
            "Core research call: {research_call}, execution: {execution_state}, confidence: {confidence_score}/100, next action: {action_label}. Priority is confirming whether the market follows the primary path."
        )];
        if conditional_bullish || matches!(core_research_call, CoreResearchCall::BuyOnConfirmation) {
            parts[0] = if matches!(decision_view.action, DecisionAction::ProbePosition) {
                format!(
                    "Research call remains buy-on-confirmation, execution still conditional. Confidence: {confidence_score}/100, next: {action_label}. Focus on small probes near confirmation with strict risk control."
                )
            } else {
                format!(
                    "Research call remains buy-on-confirmation, execution still conditional. Confidence: {confidence_score}/100, next: {action_label}. Wait for the market to confirm the primary path."
                )
            };
        } else if conditional_bearish || matches!(core_research_call, CoreResearchCall::SellOnBreak) {
            parts[0] = format!(
                "Research call shifted to sell-on-break, execution still conditional. Confidence: {confidence_score}/100, next: {action_label}. Wait for breakdown or risk evidence to complete."
            );
        }
        parts.push(format!(
            "Portfolio execution rating remains {rating}, reflecting position discipline, not a return to neutral direction."
        ));
        if has_override {
            parts.push(
                "Portfolio manager conclusion was more aggressive, but final output chose stronger execution discipline before evidence closure."
                    .to_string(),
            );
        }
        if let Some(level) = confirmation
            .as_ref()
            .filter(|level| is_publishable_summary_reference(level))
        {
            parts.push(format!(
                "Key confirmation level to watch: {}.",
                normalize_reference_phrase(level)
            ));
        }
        if let Some(level) = invalidation
            .as_ref()
            .map(|item| normalize_level_phrase(item))
            .filter(|level| is_publishable_summary_reference(level))
        {
            parts.push(format!("If {level} is reached, current thesis needs downward revision."));
        }
        if let Some(target) = target
            .as_ref()
            .filter(|target| is_publishable_summary_reference(target))
        {
            parts.push(format!(
                "Target reference: {}.",
                normalize_reference_phrase(target)
            ));
        }
        if let Some(thesis) = thesis.as_ref() {
            parts.push(format!("Core thesis: {thesis}"));
        }
        if let Some(risk) =
            risk.filter(|item| !is_semantically_similar(Some(item), thesis.as_ref()))
        {
            parts.push(format!("Key risk: {risk}"));
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
                "Direction has not turned bearish: research evidence still positive, but better to wait for clearer confirmation before expanding risk exposure."
                    .to_string()
            } else if raw_anchor.is_some_and(|value| Rating::parse(value).is_bearish()) {
                "Direction has not turned bullish: research evidence still defensive, but better to wait for clearer breakdown or risk confirmation."
                    .to_string()
            } else {
                "Directional evidence still positive, but better to wait for clearer confirmation before expanding risk exposure."
                    .to_string()
            }
        } else {
            first_non_empty_sentence(&[
                self.investment_thesis.as_str(),
                self.rationale.as_str(),
                self.executive_summary.as_str(),
            ])
            .unwrap_or_else(|| {
                "Directional evidence still positive, but better to wait for clearer confirmation before expanding risk exposure."
                    .to_string()
            })
        };
        let support = strip_redundant_prefix(
            &support,
            &[
                "Direction has not turned bearish: ",
                "Direction has not turned bullish again: ",
                &format!("Final stance: {rating}. Execution action: {action}."),
                &format!(
                    "The calibrated portfolio stance stays at {rating} with execution set to {action}."
                ),
            ],
        );
        let opening = if raw_anchor.is_some_and(|value| Rating::parse(value).is_bullish()) {
            format!(
                "Direction has not turned bearish: maintaining {rating} with action {action} does not mean asset quality has weakened. At {confidence_score}/100 confidence, evidence supports selecting a primary path and waiting for clear market confirmation."
            )
        } else if raw_anchor.is_some_and(|value| Rating::parse(value).is_bearish()) {
            format!(
                "Direction has not turned bullish again: maintaining {rating} with action {action} does not mean risk has been resolved. At {confidence_score}/100 confidence, evidence supports holding the defensive framework and waiting for clearer breakdown or risk confirmation."
            )
        } else {
            format!(
                "Maintaining {rating} with action {action} does not mean asset quality has weakened. At {confidence_score}/100 confidence, evidence supports selecting a primary path and waiting for clear market confirmation."
            )
        };
        format!(
            "{opening} {} Support context: {support}",
            confirmation
                .map(|level| format!("If price can convincingly handle the {level} confirmation level, the conclusion may be upgraded."))
                .unwrap_or_else(|| "No sufficiently credible price confirmation level yet; research judgment should not be directly upgraded to position judgment.".to_string())
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
                &format!("Final recommendation converged to {rating}"),
                &format!("Final action converged to {action}"),
            ],
        );
        format!(
            "Maintaining {rating} with action {action}. This is not about avoiding judgment, but acknowledging that {confidence_score}/100 confidence is insufficient for more aggressive action. The rational approach is to define the primary path and wait for clearer confirmation. {calibration_reason}"
        )
    }

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


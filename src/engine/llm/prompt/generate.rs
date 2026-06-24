use anyhow::Context;

/// Parameters for [`LlmClient::generate_analyst_decision`].
pub struct AnalystDecisionParams<'a> {
    pub symbol: &'a str,
    pub market_type: &'a str,
    pub analysis_date: &'a str,
    pub role_key: &'a str,
    pub role_title: &'a str,
    pub role_agent: &'a str,
    pub role_brief: &'a str,
    pub available_tools: &'a [&'a str],
    pub tool_history: &'a str,
    pub extra_context: &'a [(&'a str, &'a str)],
    pub retry_hint: Option<&'a str>,
}

/// Parameters for [`LlmClient::generate_debate_turn`].
pub struct DebateTurnParams<'a> {
    pub symbol: &'a str,
    pub market_type: &'a str,
    pub analysis_date: &'a str,
    pub speaker: &'a str,
    pub stance: &'a str,
    pub mission: &'a str,
    pub context_sections: &'a [(&'a str, &'a str)],
    pub retry_hint: Option<&'a str>,
}

/// Parameters for [`LlmClient::generate_research_manager`].
pub struct ResearchManagerParams<'a> {
    pub symbol: &'a str,
    pub market_type: &'a str,
    pub analysis_date: &'a str,
    pub market_report: &'a str,
    pub fundamentals_report: &'a str,
    pub news_report: &'a str,
    pub sentiment_report: &'a str,
    pub bull_case: &'a str,
    pub bear_case: &'a str,
    pub fact_sheet: &'a str,
    pub calibration_memo: &'a str,
    pub retry_hint: Option<&'a str>,
}

/// Parameters for [`LlmClient::generate_trader_decision`].
pub struct TraderDecisionParams<'a> {
    pub symbol: &'a str,
    pub market_type: &'a str,
    pub analysis_date: &'a str,
    pub investment_plan: &'a str,
    pub bull_case: &'a str,
    pub bear_case: &'a str,
    pub research_summary: &'a str,
    pub fact_sheet: &'a str,
    pub calibration_memo: &'a str,
    pub retry_hint: Option<&'a str>,
}

/// Parameters for [`LlmClient::generate_portfolio_decision`].
pub struct PortfolioDecisionParams<'a> {
    pub symbol: &'a str,
    pub market_type: &'a str,
    pub analysis_date: &'a str,
    pub investment_plan: &'a str,
    pub trader_plan: &'a str,
    pub bull_case: &'a str,
    pub bear_case: &'a str,
    pub fact_sheet: &'a str,
    pub calibration_memo: &'a str,
    pub retry_hint: Option<&'a str>,
}

/// Parameters for [`LlmClient::generate_reflection`].
pub struct ReflectionParams<'a> {
    pub symbol: &'a str,
    pub market_type: &'a str,
    pub analysis_date: &'a str,
    pub summary: &'a str,
    pub recommendation: &'a str,
    pub rationale: &'a str,
    pub risk_assessment: &'a str,
}

impl LlmClient {
    fn compact_fact_sheet(text: &str) -> String {
        Self::bounded_text(text, 800)
    }

    fn compact_past_context(text: &str) -> String {
        Self::bounded_text(text, 600)
    }

    fn append_retry_hint(prompt: &str, retry_hint: Option<&str>) -> String {
        match retry_hint {
            Some(hint) if !hint.trim().is_empty() => {
                format!("{}\n\n{}", prompt, hint)
            }
            _ => prompt.to_string(),
        }
    }

    pub async fn generate_analyst_decision(
        &self,
        params: AnalystDecisionParams<'_>,
    ) -> anyhow::Result<GeneratedAnalystDecision> {
        let tool_protocol = Self::analyst_tool_protocol(params.role_key, params.analysis_date, params.symbol);
        let compact_extra_context = params.extra_context
            .iter()
            .map(|(title, body)| {
                (
                    *title,
                    if *title == "Past Context" {
                        Self::compact_past_context(body)
                    } else {
                        Self::bounded_text(body, 1200)
                    },
                )
            })
            .collect::<Vec<_>>();
        let compact_extra_context_refs = compact_extra_context
            .iter()
            .map(|(title, body)| (*title, body.as_str()))
            .collect::<Vec<_>>();
        let prompt = format!(
            "You are {role_agent} in the TradingAgents workflow.\n\
             Return strict JSON only.\n\n\
             {instrument}\n\
             Analysis Date: {analysis_date}\n\
             Role Key: {role_key}\n\
             Role Title: {role_title}\n\
             Role Mission: {role_brief}\n\n\
             {desk_directive}\n\n\
             Available tools: {available_tools}\n\
             Tool usage protocol:\n{tool_protocol}\n\n\
             Tool history and prior observations:\n{tool_history}\n\n\
             Additional context:\n{extra_context}\n\n\
             Decide whether you need another tool call or whether you have enough evidence to finalize the analyst report.\n\
             If you need a tool, set `action` to `tool`, choose exactly one supported `tool_name`, and provide `tool_arguments` as a JSON object.\n\
             If you can finalize, set `action` to `finalize` and provide `final_report` matching the required role report schema.\n\
             Never output markdown fences. Do not invent tool outputs.\n\
             For fundamentals analysis, do not finalize from a single overview snapshot when the ratios or statement scope may be inconsistent; fetch statement-level confirmation first.\n\
             Your report must stay tightly grounded in the fetched evidence window and must not substitute generic finance filler for evidence.\n\
             `summary` must be one concise desk conclusion sentence.\n\
             `detail` must be a compact evidence packet focused on actionable discipline: specific price levels, indicator readings, trigger conditions, and invalidation rules -- not directional speculation. Cover key numbers, causal read-through, what changed, and what would invalidate the desk view.\n\
             If evidence is sparse, say exactly what is missing and how that limits conviction, instead of pretending certainty.\n\n\
             Required top-level JSON fields only:\n\
             action, reasoning, final_report, tool_name, tool_arguments.\n\
             `action` must be exactly `tool` or `finalize`.\n\
             `reasoning` must explain why another tool call is needed or why evidence is sufficient.\n\
             When `action=tool`, `tool_name` must be one of: {available_tools}.\n\
             When `action=finalize`, `final_report` must contain: key, title, agent, summary, detail, evidence_points, up_probability, down_probability, sideways_probability, confidence, rationale, next_steps, risks.\n\
             `up_probability`, `down_probability`, `sideways_probability` are numbers 0-1 summing to ~1.0. BE DECISIVE — timid probabilities are unacceptable. If bearish: down=0.50-0.70, up=0.10-0.25. If bullish: up=0.50-0.70, down=0.10-0.25. Only use sideways>0.40 when evidence is genuinely mixed. A clear directional lean with moderate probabilities (0.30/0.35/0.35) is a failure of analysis.\n\
             `evidence_points` must be 3-6 concrete items, not empty abstractions.",
            role_agent = params.role_agent,
            instrument = Self::instrument_context(params.symbol, params.market_type),
            analysis_date = params.analysis_date,
            role_key = params.role_key,
            role_title = params.role_title,
            role_brief = params.role_brief,
            desk_directive = Self::role_directive(params.role_key),
            available_tools = params.available_tools.join(", "),
            tool_protocol = tool_protocol,
            tool_history = if params.tool_history.trim().is_empty() {
                "No tool has been called yet.".to_string()
            } else {
                params.tool_history.to_string()
            },
            extra_context = Self::extra_context_block(&compact_extra_context_refs),
        );
        let prompt = Self::append_retry_hint(&prompt, params.retry_hint);
        let content = self.generate(&prompt).await?;
        let parsed = parse::parse_generated_analyst_decision(&content)
            .with_context(|| format!("failed to parse analyst decision JSON: {content}"))?;
        parse::validate_analyst_decision(&parsed, &content);
        Ok(parsed)
    }

    pub async fn generate_debate_turn(
        &self,
        params: DebateTurnParams<'_>,
    ) -> anyhow::Result<GeneratedDebateTurn> {
        let prompt = format!(
            "You are {speaker} in the TradingAgents workflow.\n\
             Return strict JSON only.\n\n\
             {instrument}\n\
             Analysis Date: {analysis_date}\n\
             Stance: {stance}\n\
             Mission: {mission}\n\n\
             {role_instruction}\n\n\
             Debate context:\n\
             {context}\n\n\
             Write as a live participant in a high-level investment committee debate, not as a neutral summarizer.\n\
             Directly answer the strongest opposing points when they exist.\n\
             Prioritize edge, disconfirmation, and the most decision-relevant evidence over broad narration.\n\
             State what assumption the opponent is making wrong, which evidence matters most, and what market outcome would prove your case wrong.\n\
             Do not invent unavailable data.\n\n\
             Required top-level JSON fields only:\n\
             speaker, stance, response, confidence, evidence_points, risks.\n\
             `speaker` must be exactly `{speaker}`.\n\
             `stance` must be exactly `{stance}`.\n\
             `response` must be compact: 2-4 short paragraphs or bullets focused on the decisive disagreement.\n\
             `evidence_points` should contain 2-5 concise evidence bullets.\n\
             `risks` should contain 1-3 ways your argument could fail.",
            speaker = params.speaker,
            instrument = Self::instrument_context(params.symbol, params.market_type),
            analysis_date = params.analysis_date,
            stance = params.stance,
            mission = params.mission,
            role_instruction = Self::debate_directive(params.speaker),
            context = Self::bounded_context_block(params.context_sections, 500, 1800),
        );
        let prompt = Self::append_retry_hint(&prompt, params.retry_hint);
        let content = self.generate(&prompt).await?;
        let parsed = parse::parse_generated_debate_turn(&content)
            .with_context(|| format!("failed to parse debate turn JSON: {content}"))?;
        parse::validate_debate_turn(&parsed, &content);
        Ok(parsed)
    }

    pub async fn generate_research_manager(
        &self,
        params: ResearchManagerParams<'_>,
    ) -> anyhow::Result<GeneratedResearchManager> {
        let prompt = Self::research_manager_prompt(&params);
        let prompt = Self::append_retry_hint(&prompt, params.retry_hint);
        let content = self.generate(&prompt).await?;
        parse::parse_generated_research_manager(&content)
            .with_context(|| format!("failed to parse research manager JSON: {content}"))
    }

    pub async fn generate_trader_decision(
        &self,
        params: TraderDecisionParams<'_>,
    ) -> anyhow::Result<GeneratedTraderDecision> {
        let prompt = format!(
            "You are the Trader in the TradingAgents workflow.\n\
             Return strict JSON only.\n\n\
             {instrument}\n\
             Analysis Date: {analysis_date}\n\n\
             Reliability and calibration memo:\n{calibration_memo}\n\n\
             Research plan:\n{investment_plan}\n\n\
             Bull Researcher Case:\n{bull_case}\n\n\
             Bear Researcher Case:\n{bear_case}\n\n\
             Summary Context:\n{research_summary}\n\n\
             Canonical structured facts and data limits:\n{fact_sheet}\n\n\
             Turn the research conclusion into a disciplined execution plan with clear rules.
             The output must provide trading discipline: specific entry triggers, position sizing rules, stop-loss levels, and scenario-based action plans -- not just a directional call.
             Every decision must be backed by concrete evidence and explicit conditions for action vs inaction.\n\
             Be specific about whether the proposed action should be immediate, staged, or conditional on confirmation.\n\
             If the evidence is balanced, explain why the trader should wait instead of forcing activity.\n\
             Explicitly distinguish long-term structural strength from short-term executable confirmation.\n\
             If execution proof is incomplete, prefer Hold and name the concrete trigger checklist required before capital should be deployed.
             CRITICAL DISCIPLINE GATE: (1) ALL-NEUTRAL CALIBRATION RULE: If the calibration memo shows historical resolved setups exist AND are entirely neutral (resolved > 0, ALL neutral, zero bullish AND zero bearish), the trader action MUST be Hold. The position_sizing field MUST be an empty string. The entry_price field MUST also be an empty string. Scenario paths must NOT contain any short/sell position sizing, short entry levels, or short stop-losses. Instead, state in reasoning: historical calibration is entirely neutral, no directional entry is warranted. HOWEVER, if the calibration memo shows NO historical calibration data is available (resolved=0, no data), this Hold-forcing restriction does NOT apply. In that case, base the action entirely on current market evidence: technicals, fundamentals, news, and sentiment. You MAY recommend Buy or Sell when current evidence strongly supports a directional view, even without historical calibration backing. The key discipline is: ground every conclusion in concrete evidence and specific price levels, not in the absence of history. (2) If setup quality is low (conditional/watchlist), the trader_plan must state that at most an observation position is warranted, not a full conviction trade. Do NOT provide precise entry/stop/sizing for low-quality setups without a prominent caveat that the setup does not warrant standard execution. (3) UNIFIED FRAMEWORK: The trader_plan must provide one single discipline anchor for both holders and prospective buyers -- define a unified zone: below X = exit/avoid, between X-Y = hold/observe, above Y = confirmed entry. Do NOT give conflicting anchors for different position states. (4) TECHNICAL CROSS-VALIDATION: When citing MACD histogram and OBV together, you MUST cross-validate them: if MACD histogram is expanding positive while OBV is not making new lows, note this as potential bottom divergence (bullish signal), not just volume outflow. Do NOT cite MACD and OBV as independent bearish signals without checking whether they contradict each other. (5) CONFIRMATION LEVEL ANCHORING: When setting a confirmation level (e.g. entry_price, confirmation_level), it MUST be anchored to a named technical indicator with the specific value cited. Do NOT set a round-number level without explaining which indicator it maps to. (6) VALUATION DATA CONSISTENCY: If you state that PE/valuation data is distorted or unreliable due to reporting period issues, do NOT then use that same PE number as a core risk anchor. Either correct the data (e.g. annualize a quarterly figure) or exclude it from risk framing. You may still cite other fundamental risks (cash flow, margins) independently. (7) POSITION CONSISTENCY: When early probe is not allowed, the position_sizing field MUST be empty or 0%%. Do NOT write 5%% or any non-zero percentage unless explicitly conditioned on a trigger firing first. This applies to ALL position sizing fields in the output, including scenario paths. (8) RISK-REWARD RATIO CONSISTENCY: Every risk-reward or profit-loss ratio cited must include its calculation basis: upside target price, downside risk price, and whether the target is conditional. Do NOT cite a theoretical post-breakout ratio alongside a current-state ratio without explaining the difference. When the target depends on a confirmation level being breached first, present TWO ratios: (a) current-to-confirmation (the unconfirmed gap risk) and (b) confirmation-to-target (the post-breakout space). Do NOT merge them into a single misleading ratio. (9) ENTRY vs INVALIDATION DISTINCTION: `entry_price` and `stop_loss` MUST be different values. Do NOT set the stop-loss at the same level as the entry price. If the setup is a pullback-to-support play, entry IS the support level and stop is BELOW the support (e.g. 1-2%% below). (10) VOLUME DIVERGENCE WEIGHTING: When OBV shows extreme divergence from price (significantly negative while price is near highs, or diverging >5 trading days), the trader_plan MUST explicitly address whether sufficient buying volume exists at the proposed pullback entry level. Do NOT recommend buying on a pullback without addressing OBV. If OBV is extremely bearish, this must increase the required confirmation threshold or reduce position sizing.\n\
             If the setup is conditional rather than immediately executable, still provide the concrete breakout or pullback trigger level in `entry_price` whenever a real level exists.\n\
             Keep the reasoning tied to analyst evidence, invalidation levels, and practical execution discipline.\n\
             Treat the reliability and calibration memo as informative operating context. When no historical calibration data is available, do NOT let the absence of history default you to Hold -- base the action entirely on current evidence and technicals.\n\n\
             Required top-level JSON fields only:\n\
             action, reasoning, trader_plan, entry_price, stop_loss, confirmation_level, target_reference, target_condition, time_horizon, position_sizing, execution_trigger_checklist, blocking_gaps.\n\
            `action` must be exactly one of Buy, Hold, Sell.\n\
            `reasoning` must be compact and explain execution logic, triggers, invalidation conditions, and the blocking proof points.\n\
            `trader_plan` should be a concise trader hand-off in Markdown, suitable for storage and display.\n\
            `stop_loss` MUST always be provided when any invalidation level or support zone exists in the evidence -- this is a required field, not optional. `entry_price`, `confirmation_level`, `target_reference`, `target_condition`, `time_horizon`, and `position_sizing` should be provided when the available context supports them; otherwise leave them null or empty.\n\
            `execution_trigger_checklist` must be a concise array of 2-6 concrete execution triggers when the action is conditional or Hold.\n\
            `blocking_gaps` must be a concise array of concrete missing proof points that still block execution.",
            instrument = Self::instrument_context(params.symbol, params.market_type),
            analysis_date = params.analysis_date,
            calibration_memo = Self::bounded_text(params.calibration_memo, 800),
            investment_plan = Self::bounded_text(params.investment_plan, 1000),
            bull_case = Self::bounded_text(params.bull_case, 700),
            bear_case = Self::bounded_text(params.bear_case, 700),
            research_summary = Self::bounded_text(params.research_summary, 600),
            fact_sheet = Self::compact_fact_sheet(params.fact_sheet),
        );
        let prompt = Self::append_retry_hint(&prompt, params.retry_hint);
        let content = self.generate(&prompt).await?;
        let parsed = parse::parse_generated_trader_decision(&content)
            .with_context(|| format!("failed to parse trader decision JSON: {content}"))?;
        parse::validate_trader_decision(&parsed, &content);
        Ok(parsed)
    }

    pub async fn generate_portfolio_decision(
        &self,
        params: PortfolioDecisionParams<'_>,
    ) -> anyhow::Result<GeneratedPortfolioDecision> {
        let prompt = Self::portfolio_decision_prompt(&params);
        let prompt = Self::append_retry_hint(&prompt, params.retry_hint);
        let content = self.generate(&prompt).await?;
        let parsed = parse::parse_generated_portfolio_decision(&content)
            .with_context(|| format!("failed to parse portfolio decision JSON: {content}"))?;
        parse::validate_portfolio_decision(&parsed, &content);
        Ok(parsed)
    }

    pub async fn generate_reflection(
        &self,
        params: ReflectionParams<'_>,
    ) -> anyhow::Result<String> {
        let prompt = format!(
            "You are a senior trading reviewer.\n\
             Return strict JSON only. All fields must be structured data, not narrative text.\n\
             Return valid JSON only with exactly these keys:\n\
             - strongest_part\n\
             - weakest_uncertainty_or_missing_evidence\n\
             - next_lesson_for_next_run\n\n\
             {instrument}\n\
             Analysis Date: {analysis_date}\n\
             Recommendation: {recommendation}\n\
             Risk Assessment: {risk_assessment}\n\n\
             Summary:\n{summary}\n\n\
             Rationale:\n{rationale}\n\n\
             Write a concise but concrete reflection covering:\n\
             1. the strongest part of this analysis\n\
             2. the weakest uncertainty or missing evidence\n\
             3. the next lesson to apply in the next run\n\
             Keep it evidence-driven, specific, and free of filler.\n\
             Do not add any extra keys or markdown fences.",
            instrument = Self::instrument_context(params.symbol, params.market_type),
            analysis_date = params.analysis_date,
            recommendation = params.recommendation,
            risk_assessment = params.risk_assessment,
            summary = params.summary,
            rationale = params.rationale,
        );
        self.generate(&prompt).await
    }

}

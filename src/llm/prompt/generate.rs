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
             If you need tools, set `action` to `tool` and provide `tool_calls` as an array of {{tool_name, tool_arguments}} objects. Request ALL needed tools at once — do not request one at a time. Each tool_name must be one of: {available_tools}.\n\
             If you can finalize, set `action` to `finalize` and provide `final_report` matching the required role report schema.\n\
             Never output markdown fences. Do not invent tool outputs.\n\
             For fundamentals analysis, do not finalize from a single overview snapshot when the ratios or statement scope may be inconsistent; fetch statement-level confirmation first.\n\
             Your report must stay tightly grounded in the fetched evidence window and must not substitute generic finance filler for evidence.\n\
             `summary` must be one concise desk conclusion sentence.\n\
             `detail` must be a compact evidence packet focused on actionable discipline: specific price levels, indicator readings, trigger conditions, and invalidation rules -- not directional speculation. Cover key numbers, causal read-through, what changed, and what would invalidate the desk view.\n\
             If evidence is sparse, say exactly what is missing and how that limits conviction, instead of pretending certainty.\n\n\
             Required top-level JSON fields only:\n\
             action, reasoning, final_report, tool_calls.\n\
             `action` must be exactly `tool` or `finalize`.\n\
             `reasoning` must explain why tool calls are needed or why evidence is sufficient.\n\
             When `action=tool`, `tool_calls` must be an array of objects with `tool_name` (one of: {available_tools}) and `tool_arguments` (JSON object).\n\
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
             CRITICAL: Do NOT default to Hold when evidence leans directional. If technical indicators and/or fundamentals clearly favor one side, recommend that direction (Buy or Sell). Hold is ONLY appropriate when bull and bear arguments are genuinely of equal weight.\n\
             DECISION MATRIX (apply strictly):\n\
             - Technical bullish + Fundamentals healthy => Buy/Overweight\n\
             - Technical bearish + Fundamentals deteriorating => Sell/Underweight\n\
             - Genuinely balanced evidence => Hold\n\
             Explicitly distinguish long-term structural strength from short-term executable confirmation.\n\
             If execution proof is incomplete, name the concrete trigger checklist required before capital should be deployed -- but still give a directional recommendation if evidence leans one way.\n\
             CRITICAL DISCIPLINE GATE: (1) ALL-NEUTRAL CALIBRATION: If calibration shows ALL resolved setups are neutral (resolved > 0, all neutral), trader action MUST be Hold. If NO historical data exists (resolved=0), base action on current evidence -- you MAY recommend Buy or Sell. (2) LOW SETUP QUALITY: If setup quality < 50, observation-only. (3) UNIFIED FRAMEWORK: One discipline anchor for holders and buyers. (4) ENTRY vs STOP: entry_price and stop_loss MUST be different values.\n\
             If the setup is conditional rather than immediately executable, still provide the concrete breakout or pullback trigger level in `entry_price` whenever a real level exists.\n\
             Keep the reasoning tied to analyst evidence, invalidation levels, and practical execution discipline.\n\
             Treat the reliability and calibration memo as informative operating context. When no historical calibration data is available, do NOT let the absence of history default you to Hold -- base the action entirely on current evidence and technicals.\n\n\
             Required top-level JSON fields only:\n\
             action, reasoning, trader_plan, entry_price, stop_loss, confirmation_level, target_reference, target_condition, time_horizon, position_sizing, execution_trigger_checklist, blocking_gaps.\n\
            `action` must be exactly one of Buy, Hold, Sell.\n\
            `reasoning` must be compact and explain execution logic, triggers, invalidation conditions, and the blocking proof points.\n\
            `trader_plan` should be a concise trader hand-off in Markdown, suitable for storage and display.\n\
            FIELD REQUIREMENTS (strict):\n\
            - Buy/Sell actions: `entry_price`, `stop_loss`, and `time_horizon` are REQUIRED. Leaving any of these empty is a schema violation.\n\
            - Hold action: `entry_price` and `stop_loss` may be empty, but `stop_loss` should still be provided if any invalidation level exists in the evidence.\n\
            - `confirmation_level`, `target_reference`, `target_condition`, `position_sizing`: provide when evidence supports them; null/empty is acceptable.\n\
            - `entry_price` and `stop_loss` MUST be different numeric values.\n\
            CONFIRMATION SIMPLICITY RULE: `confirmation_level` must be ONE primary price level with at most ONE supporting condition. Format: \"PRICE (INDICATOR) — brief condition\". Do NOT chain multiple indicator conditions (MACD, KDJ, RSI, etc.) into confirmation_level — those belong in `execution_trigger_checklist` as separate items.\n\
            NOISE FILTER RULE: When `confirmation_level` is within 2xATR(14) of the current price, intraday price noise can falsely trigger it. In such cases, confirmation MUST require DAILY CLOSE above the level (not just intraday touch), plus volume >= 1.5x 20-day average. Format confirmation_level as: \"PRICE — daily close above with volume >= 1.5x 20d avg\". Do NOT use intraday-only confirmation when the level is within 2xATR of current price.\n\
            CONFIRMATION DISTANCE RULE (MANDATORY): confirmation_level MUST be at least 2xATR(14) from current_price. This is a hard constraint, not advisory. If the nearest technical level (SMA, Bollinger, resistance) is within 2xATR of current price, you MUST NOT use it as confirmation. Instead, set confirmation_level to indicate deferral and in execution_trigger_checklist state that confirmation requires price to first pull back and then establish a new breakout level with volume. Example: if current=212, ATR=7, and Bollinger upper=214, do NOT set confirmation=214 because it is only 0.3xATR away and will trigger on noise.\n\
            HOLDING DISCIPLINE RULE: When entry_price and confirmation_level are different levels, define explicit holding rules in `trader_plan` for the zone between them:\n\
            (1) If price moves from entry toward confirmation: hold the position, do NOT add, do NOT take early profit.\n\
            (2) If price touches confirmation_level but does NOT close above it with volume: do NOT treat as confirmed — wait for the next trading day's close.\n\
            (3) If price closes above confirmation_level with volume >= 1.5x 20d avg: confirmation triggered, may add.\n\
            (4) If price falls back below entry_price after entry: exit or reduce position (capital preservation).\n\
            The position between entry and confirmation is an \"observation position\", not a \"trend position\" — do not let unrealized P&L influence the decision.\n\
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

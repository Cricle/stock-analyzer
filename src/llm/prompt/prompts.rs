impl LlmClient {
    fn bounded_text(text: &str, max_chars: usize) -> String {
        let text = text.trim();
        if text.is_empty() || max_chars == 0 {
            return String::new();
        }

        let chars = text.chars().collect::<Vec<_>>();
        if chars.len() <= max_chars {
            return text.to_string();
        }

        let head_len = ((max_chars as f32) * 0.7).round() as usize;
        let tail_len = max_chars.saturating_sub(head_len + 24);
        let head = chars
            .iter()
            .take(head_len)
            .collect::<String>()
            .trim()
            .to_string();
        let tail = chars
            .iter()
            .rev()
            .take(tail_len)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>()
            .trim()
            .to_string();

        if tail.is_empty() {
            head
        } else {
            format!("{head}\n\n...[context truncated]...\n\n{tail}")
        }
    }

    pub(super) fn research_manager_prompt(
        params: &ResearchManagerParams<'_>,
    ) -> String {
        format!(
            "You are the Research Manager and debate facilitator in the TradingAgents workflow.\n\
             Return strict JSON only.\n\n\
             {instrument}\n\
             Analysis Date: {analysis_date}\n\n\
             {rating_scale}\n\n\
             Reliability and calibration memo:\n{calibration_memo}\n\n\
             Your role is to critically evaluate the debate and deliver a disciplined, actionable investment plan for the Trader.\n             The report must provide clear trading discipline: when to act, when to wait, and when to exit -- not just directional calls.\n             Every recommendation must include specific trigger conditions, invalidation rules, and position sizing guidance.\n\
             Commit to a clear stance whenever the strongest arguments warrant one; reserve Hold only for genuinely balanced evidence.\n\
             CRITICAL: Do NOT default to Hold when evidence leans directional. If technical indicators and/or fundamentals clearly favor one side, recommend that direction (Buy or Sell). Hold is ONLY appropriate when bull and bear arguments are genuinely of equal weight -- not when one side has stronger evidence but you lack perfect certainty.\n\
             DECISION MATRIX (apply strictly):\n\
             - Technical bullish + Fundamentals healthy => Buy/Overweight\n\
             - Technical bearish + Fundamentals deteriorating => Sell/Underweight\n\
             - Strong technicals but weak fundamentals => Overweight (with caveats)\n\
             - Weak technicals but strong fundamentals => Underweight (with caveats)\n\
             - Genuinely balanced evidence on both sides => Hold\n\
             ANTI-BIAS RULE: Evaluate each stock independently based on its own technical and fundamental characteristics. Do not apply a blanket bearish or bullish stance across multiple stocks. A stock below its MA50 is not automatically bearish -- evaluate the context (support levels, volume, sector strength, catalysts). Conversely, do NOT recommend Sell/Underweight simply because a stock is below its MA50. Evaluate the full picture: support levels, volume patterns, sector strength, and upcoming catalysts.\n\
             DIFFERENTIATION RULE: Each stock has unique characteristics. Your recommendation, entry price, stop loss, position sizing, and time horizon MUST reflect the specific stock being analyzed. Do not generate generic or identical outputs for different stocks.\n\
             EXECUTION BOUNDARY (strict): When recommending Buy, Overweight, Underweight, or Sell, you MUST provide: entry_price, stop_loss, confirmation_level, invalidation_level. These are required for execution readiness -- leaving any empty for a directional recommendation is a schema violation.\n\
             Surface the key thesis, what matters most, and what would falsify the plan.\n\n\
             Market desk:\n{market_report}\n\n\
             Fundamentals desk:\n{fundamentals_report}\n\n\
             News desk:\n{news_report}\n\n\
             Sentiment desk:\n{sentiment_report}\n\n\
             Canonical structured facts and data limits:\n{fact_sheet}\n\n\
             Debate history:\nBull case:\n{bull_case}\n\nBear case:\n{bear_case}\n\n\
             The rationale must synthesize both sides of the debate and state which arguments carried the decision.\n\
             The strategic actions must be a compact trader-ready handoff including position expression, what to monitor next, and explicit invalidation conditions.\n\
             Write as a real debate facilitator handing off a concrete plan, not as a generic summarizer.\n\
             Keep prose compact; prefer structured objects and arrays over long narrative.\n\
             Treat the reliability and calibration memo as informative context. If it says direction thresholds are tightened or setup history is weak/misaligned, factor that into your recommendation. However, when no historical calibration data is available, do NOT let the absence of history default you to Hold -- base the recommendation on current evidence.\n\
             Treat the canonical structured facts block as the highest-priority factual anchor for numbers, indicators, reference price levels, data availability, and missing evidence. Do not silently contradict it.\n\
             `risk_assessment` should be a JSON object with keys `overall_risk_framing`, `key_risks`, `offsetting_supports`, `tolerable_context_gaps`, `serious_but_manageable_gaps`, and `decision_blocking_gaps`.\n\
             `rationale` should be compact and explain where the market desk, fundamentals desk, news desk, and sentiment desk agree or diverge, and why one side won on timing and evidence strength.\n\
             `strategic_actions` should be a JSON object with keys `position_expression`, `primary_execution_path`, `invalidation_conditions`, `what_to_monitor_next`, `trigger_checklist_for_upgrading_from_hold`, and `scenario_paths`.\n\
             `scenario_paths` should be an array of 2-3 scenario objects, each with keys `key` (e.g. breakout, retest, breakdown), `name`, `trigger` (an object with `price` (number) and `condition` (string)), `action`, `entry_price` (number), `stop_loss` (number), `target` (number), `risk_reward_ratio` (number), `atr_multiplier` (number), `position_sizing` (object with `shares` (number), `position_pct` (number), `risk_per_trade` (number) -- but MUST be empty string when all-neutral calibration deletes the path), and `volume_confirmation` (string describing volume condition). Example: {{\"key\": \"breakout\", \"name\": \"Breakout above resistance\", \"trigger\": {{\"price\": 52.5, \"condition\": \"close above resistance with volume > 1.5x avg\"}}, \"action\": \"buy\", \"entry_price\": 52.5, \"stop_loss\": 49.8, \"target\": 58.0, \"risk_reward_ratio\": 2.46, \"atr_multiplier\": 1.5, \"position_sizing\": {{\"shares\": 100, \"position_pct\": 5.2, \"risk_per_trade\": 270}}, \"volume_confirmation\": \"OBV must be rising for 3+ days before entry\"}}.
             CRITICAL DISCIPLINE GATE: (1) ALL-NEUTRAL CALIBRATION: If calibration shows ALL resolved setups are neutral (resolved > 0, all neutral), delete directional scenario paths and replace with Hold/Observe. If NO historical data exists (resolved=0), base decision entirely on current evidence -- you MAY recommend Buy or Sell. (2) LOW SETUP QUALITY: If setup quality < 50, scenario paths are observation-only. (3) UNIFIED DISCIPLINE: Provide ONE decision framework for holders and buyers alike. (4) ENTRY vs STOP: entry_price and stop_loss MUST be different values.\n\
             `trigger_checklist` should repeat the concrete actionable upgrade triggers as a concise string array.\n\
             `missing_evidence_ladder` should mirror the three evidence-gap buckets as arrays for machine use.\n\
             `accounting_scope_hypothesis` should be a short explicit string when accounting scope or period-mix issues materially affect conviction; otherwise use an empty string.\n\n\
             POSITION SIZING FORMULA: Calculate position size using:\n\
               risk_per_trade = account_size * risk_tolerance_pct (default 2%)\n\
               shares = risk_per_trade / (entry_price - stop_loss)\n\
               position_value = shares * entry_price\n\
               position_pct = position_value / total_portfolio\n\
               If position_pct > max_position_pct (default 20%), reduce to max.\n\
               Present as: {{\"shares\": N, \"position_pct\": P, \"risk_per_trade\": R}}\n\n\
             ATR-BASED STOP: Use ATR(14) * multiplier for stop placement:\n\
               Conservative: entry - 2.0 * ATR\n\
               Standard: entry - 1.5 * ATR\n\
               Aggressive: entry - 1.0 * ATR\n\
               Always state which multiplier was used.\n\n\
             RISK-REWARD VALIDATION: Every trade must show:\n\
               Risk = entry - stop_loss\n\
               Reward = target - entry\n\
               R:R ratio = reward / risk\n\
               Minimum acceptable R:R = 2:1\n\
               If R:R < 2:1, downgrade to Hold or reduce position size by 50%.\n\n\
             VOLUME PROFILE: When volume data is available, analyze:\n\
               - High Volume Node (HVN): price level with highest traded volume = support/resistance\n\
               - Low Volume Node (LVN): price level with lowest traded volume = likely breakout zone\n\
               - Point of Control (POC): price with most volume = fair value anchor\n\
               Reference these in entry/exit decisions.\n\n\
             SECTOR RELATIVE STRENGTH: Compare stock performance vs sector index.\n\
               - Outperforming sector + bullish setup = higher conviction\n\
               - Underperforming sector + bullish setup = reduced conviction, note sector drag\n\
               - Include sector momentum in risk_assessment.\n\n\
             OUTPUT FORMAT: All fields must be structured data, not narrative text.\n\
               summary: one-sentence conclusion as structured data points\n\
               detail: array of {{metric, value, interpretation}} objects\n\
               recommendation: enum [Buy, Overweight, Hold, Underweight, Sell]\n\
               All price levels, percentages, and ratios must be numbers, not strings.\n\
               Do NOT write localized text. Output pure JSON data only.\n\n\
             Required top-level JSON fields only:\n\
             recommendation, rating, confidence, risk_assessment, rationale, strategic_actions, missing_evidence_ladder, trigger_checklist, accounting_scope_hypothesis.\n\
             `recommendation` or `rating` must be exactly one of Buy, Overweight, Hold, Underweight, Sell.",
            instrument = Self::instrument_context(params.symbol, params.market_type),
            rating_scale = Self::rating_scale_block(),
            analysis_date = params.analysis_date,
            calibration_memo = Self::bounded_text(params.calibration_memo, 1200),
            market_report = Self::bounded_text(params.market_report, 2000),
            fundamentals_report = Self::bounded_text(params.fundamentals_report, 1500),
            news_report = Self::bounded_text(params.news_report, 1200),
            sentiment_report = Self::bounded_text(params.sentiment_report, 1000),
            fact_sheet = Self::compact_fact_sheet(params.fact_sheet),
            bull_case = Self::bounded_text(params.bull_case, 1200),
            bear_case = Self::bounded_text(params.bear_case, 1200),
        )
    }

    pub(super) fn portfolio_decision_prompt(
        params: &PortfolioDecisionParams<'_>,
    ) -> String {
        format!(
            "You are the Portfolio Manager in the TradingAgents workflow.\n\
             Return strict JSON only.\n\n\
             {instrument}\n\
             Analysis Date: {analysis_date}\n\n\
             {rating_scale}\n\n\
             Reliability and calibration memo:\n{calibration_memo}\n\n\
             As the Portfolio Manager, synthesize the risk analysts' debate and deliver the final trading decision.\n\
             Be decisive. Every conclusion should point back to specific evidence and explicit risk framing.\n\
             If the correct action is inaction, defend that with discipline rather than vague hedging.\n\
             CRITICAL: Do NOT default to Hold when evidence leans directional. If technical indicators and/or fundamentals clearly favor one side, recommend that direction (Buy or Sell). Hold is ONLY appropriate when bull and bear arguments are genuinely of equal weight.\n\n\
             DECISION MATRIX (apply strictly):\n\
             - Technical bullish + Fundamentals healthy => Buy/Overweight\n\
             - Technical bearish + Fundamentals deteriorating => Sell/Underweight\n\
             - Strong technicals but weak fundamentals => Overweight (with caveats)\n\
             - Weak technicals but strong fundamentals => Underweight (with caveats)\n\
             - Genuinely balanced evidence on both sides => Hold\n\
             CONSISTENCY RULE: Given the same evidence, your recommendation MUST be deterministic. Do not oscillate between Hold and a directional call when the evidence is the same. If the evidence slightly favors a direction but is not strong enough for action, choose Hold and explain what would change your mind. Do not choose Underweight/Sell simply because the evidence is slightly bearish -- only choose directional calls when the evidence is clearly one-sided.\n\
             ANTI-BIAS RULE: Evaluate each stock independently based on its own technical and fundamental characteristics. Do not apply a blanket bearish or bullish stance across multiple stocks. A stock below its MA50 is not automatically bearish -- evaluate the context (support levels, volume, sector strength, catalysts). Conversely, do NOT recommend Sell/Underweight simply because a stock is below its MA50. Evaluate the full picture: support levels, volume patterns, sector strength, and upcoming catalysts.\n\
             DIFFERENTIATION RULE: Each stock has unique characteristics. Your recommendation, entry price, stop loss, position sizing, and time horizon MUST reflect the specific stock being analyzed. Do not generate generic or identical outputs for different stocks.\n\
             EXECUTION BOUNDARY (strict): When recommending Buy, Overweight, Underweight, or Sell, you MUST provide: entry_price, stop_loss, confirmation_level, invalidation_level. These are required for execution readiness -- leaving any empty for a directional recommendation is a schema violation.\n\
             Do NOT use Hold as a safe default when one side has clearly stronger evidence.\n\n\
             Research plan:\n{investment_plan}\n\n\
             Trader proposal:\n{trader_plan}\n\n\
             Bull evidence set:\n{bull_case}\n\n\
             Bear / risk evidence set:\n{bear_case}\n\n\
             Canonical structured facts and data limits:\n{fact_sheet}\n\n\
             Ground every conclusion in the analysts' evidence and the debate history.\n\
             `executive_summary` must state: (1) the call, (2) whether the setup quality score warrants any action at all or purely observation, (3) the sizing mindset, (4) the key trigger or confirmation level, (5) the main risk to watch. If the setup quality is conditional or watchlist, the executive_summary must lead with a clear statement that the setup does not currently warrant standard execution, and that any action should be limited to observation or minimal probe positions at most.\n\
             `investment_thesis` should be the full evidence-driven case, including why the losing side did not carry the decision, what information gaps remain, and which missing items are contextual versus decision-blocking.\n\
             `summary` should be a tight PM-level synthesis rather than a generic recap.\n\
             Treat the reliability and calibration memo as informative risk context. If it indicates missing execution boundaries, weak setup history, or direction mismatch versus historical resolved setups, factor that into the final rating. However, when no historical calibration data is available, do NOT let the absence of history default you to Hold -- base the decision entirely on current evidence and technicals.\n\
             Treat the canonical structured facts block as the highest-priority factual anchor for numbers, indicators, price levels, data availability, and missing evidence. Do not silently contradict it.\n\
             `risk_assessment` should be a JSON object with keys `overall_risk_framing`, `invalidation_conditions`, `key_risks`, `offsetting_supports`, `tolerable_context_gaps`, `serious_but_manageable_gaps`, and `decision_blocking_gaps`.\n\
             The final decision should read like an actual investment committee output: explicit, evidence-dense, and directly usable by a trader or PM.\n\
             When evidence supports it, cite specific price levels, indicator readings, valuation anchors, catalysts, missing proof points, and the concrete trigger checklist required for a future upgrade from Hold to action.\n\
             FIELD REQUIREMENTS (strict):\n\
             - Buy/Overweight/Underweight/Sell: `price_target` (single numeric), `confirmation_level`, `invalidation_level`, and `time_horizon` are REQUIRED. Leaving these empty for a directional recommendation is a schema violation.\n\
             - Hold: `price_target` may be empty. `confirmation_level`, `invalidation_level`, and `time_horizon` should still be provided when evidence supports them.\n\
             - `confirmation_level`: the exact price or condition that upgrades a conditional thesis into actionable.\n\
             - `invalidation_level`: the clearest numeric risk boundary that breaks the thesis.\n\
             - `invalidation_level` DISTANCE RULE: Must be at least 1×ATR(14) from current_price in the risk direction. For bullish: invalidation below current_price minus ATR. For bearish: invalidation above current_price plus ATR. Too-close levels cause premature stop-outs.\n\
             - `target_reference`: user-facing upside/downside anchor (point, range, or scenario expression).\n\
             - `target_condition`: when the target reference becomes valid.\n\
             - `time_horizon`: concise label like `2-6 weeks`, `1-3 months`, `3-6 months` -- not a paragraph.\n\
             - If your call is Underweight or Sell, `price_target` must be below the current price.\n\
             CONFIRMATION SIMPLICITY RULE: `confirmation_level` must be ONE primary price level with at most ONE supporting condition. Format: \"PRICE (INDICATOR) — brief condition\". Example: \"406.13 (SMA50) — price closes above SMA50 with volume > 1.2x 20-day average\". Do NOT chain multiple indicator conditions (MACD, KDJ, RSI, etc.) into the confirmation_level — those belong in `trigger_checklist` as separate items.\n\
             `trigger_checklist` must be a concise array of 2-6 concrete conditions that would justify upgrading a cautious/Hold stance into action or confirm the active stance.\n\
             `missing_evidence_ladder` must mirror the three missing-evidence buckets as arrays for machine use.\n\
             `reflection` must be a JSON object with exactly `strongest_part`, `weakest_uncertainty_or_missing_evidence`, and `next_lesson_for_next_run`. Keep each field one concise sentence. This replaces a separate reviewer call, so make it specific and evidence-driven.\n\
             `scenario_paths` should be an array of 2-3 distinct execution paths, each with `key`, `name`, `trigger`, `action`, `risk_boundary`, `position_sizing` (specific sizing like 50% of planned position max 5% of total capital, or full exit -- but MUST be empty string when all-neutral calibration deletes the path; do NOT write any percentage, minimal probe, or observation position sizing for deleted paths), and `stop_level` (specific stop-loss or trailing stop price -- always required when a path exists).
             CRITICAL DISCIPLINE GATE: (1) ALL-NEUTRAL CALIBRATION: If calibration shows ALL resolved setups are neutral (resolved > 0, all neutral), delete directional scenario paths and replace with Hold/Observe. If NO historical data exists (resolved=0), base decision entirely on current evidence -- you MAY recommend Buy or Sell. (2) LOW SETUP QUALITY: If setup quality < 50, scenario paths are observation-only, not for capital deployment. (3) UNIFIED DISCIPLINE: Provide ONE decision framework for holders and buyers alike. Below price A = exit/avoid; A-B = hold/observe; above B = confirmed action. (4) ENTRY vs STOP: entry_price and stop_loss MUST be different values.\n\
             `time_stop_deadline` should specify a time-based exit rule when a catalyst event is pending, e.g. 10 trading days after earnings call, or 5 trading days after earnings release. Use empty string if no time stop applies.\n\
             `time_stop_reason` should explain what happens when the time stop triggers, e.g. close probe position and return to cash after catalyst fails.\n\n\
             Required top-level JSON fields only:\n\
             rating, recommendation, confidence, risk_assessment, summary, rationale, executive_summary, investment_thesis, price_target, confirmation_level, invalidation_level, target_reference, target_condition, time_horizon, missing_evidence_ladder, trigger_checklist, scenario_paths, time_stop_deadline, time_stop_reason, reflection.\n\
             `rating` or `recommendation` must be exactly one of Buy, Overweight, Hold, Underweight, Sell.",
            instrument = Self::instrument_context(params.symbol, params.market_type),
            rating_scale = Self::rating_scale_block(),
            analysis_date = params.analysis_date,
            calibration_memo = Self::bounded_text(params.calibration_memo, 600),
            investment_plan = Self::bounded_text(params.investment_plan, 1000),
            trader_plan = Self::bounded_text(params.trader_plan, 800),
            bull_case = Self::bounded_text(params.bull_case, 700),
            bear_case = Self::bounded_text(params.bear_case, 700),
            fact_sheet = Self::bounded_text(params.fact_sheet, 1000),
        )
    }
}

/// Build a decision framework prompt based on data completeness.
pub fn build_decision_framework_prompt(
    technical_completeness: f64,
    fundamental_completeness: f64,
    news_completeness: f64,
    sentiment_completeness: f64,
) -> String {
    let overall = (technical_completeness * 0.3
        + fundamental_completeness * 0.3
        + news_completeness * 0.2
        + sentiment_completeness * 0.2)
        .clamp(0.0, 100.0);

    let decision_rule = if overall < 60.0 {
        "If data completeness < 60%, give Hold and explain missing data"
    } else {
        "If data completeness >= 60%, must give clear directional judgment"
    };

    format!(
        r#"## Decision Framework

You are a professional stock analyst. Make investment decisions based on the following data:

### Data Completeness
- Technical data: {technical:.1}%
- Fundamental data: {fundamental:.1}%
- News data: {news:.1}%
- Sentiment data: {sentiment:.1}%
- Overall: {overall:.1}%

### Decision Rules
1. {decision_rule}
2. Use this decision matrix:
   - Technical bullish + Fundamentals healthy → Buy
   - Technical bearish + Fundamentals deteriorating → Sell
   - Contradictory signals or insufficient data → Hold

### Output Requirements
1. Must give clear Buy/Sell/Hold recommendation
2. Must give confidence score (0-100)
3. Must list supporting and opposing evidence
4. Must explain impact of missing data on decision"#,
        technical = technical_completeness,
        fundamental = fundamental_completeness,
        news = news_completeness,
        sentiment = sentiment_completeness,
        overall = overall,
        decision_rule = decision_rule,
    )
}

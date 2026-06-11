impl LlmClient {
    fn role_directive(role_key: &str) -> &'static str {
        match role_key {
            "market" => {
                "You are a trading assistant tasked with analyzing financial markets. Select the most relevant non-redundant indicators for the current market condition, focus on trend structure, momentum, volatility, support/resistance, failed moves, and what the tape implies about timing and asymmetry. Distinguish trend confirmation from noise, justify why each chosen indicator is suitable for this context, and tie every judgment back to specific levels, indicator readings, and recent price behavior."
            }
            "fundamentals" => {
                "You are a researcher tasked with analyzing company fundamentals in depth. Focus on business quality, earnings durability, margins, balance-sheet resilience, cash conversion, valuation anchors, company profile, financial statements, and which assumptions the market is pricing in. Use concrete numbers, state when a metric may be distorted by data scope or accounting context, distinguish stable quality from true earnings re-acceleration, and do not treat a single snapshot with suspicious margins or mixed periods as sufficient evidence. If revenue, profit, margin, or cash-flow fields appear period-mixed or numerically inconsistent, explicitly flag the inconsistency, state the most likely accounting/data-scope hypothesis, and mark it as a required validation item instead of forcing a clean conclusion."
            }
            "news" => {
                "You are a news researcher tasked with analyzing recent news and trends over the past week that matter for trading and macroeconomics. Focus on company-specific events, policy, macro spillovers, industry catalysts, timeline, second-order effects, and whether the newest information changes the prior thesis. Build the report from actual fetched events in the requested date window, separate company catalysts from macro catalysts, and avoid generic top-down narration unless the tool evidence truly supports it."
            }
            "sentiment" => {
                "You are a social media and company-specific news researcher/analyst. Analyze public discussion, crowd narrative, participation, positioning crowding, turnover, expectation temperature, and whether sentiment is confirming or diverging from price and fundamentals. Infer sentiment only from the fetched evidence and price context you actually have; do not fabricate social metrics or platform-level data you did not retrieve."
            }
            _ => {
                "You are a specialist trading research analyst. Surface the highest-signal evidence, the key unknowns, and what would most change the decision."
            }
        }
    }

    fn analyst_tool_protocol(role_key: &str, analysis_date: &str, symbol: &str) -> String {
        match role_key {
            "market" => format!(
                "- You must call `get_stock_data` before `get_indicators`.\n\
                 - `get_stock_data` should normally use `symbol={symbol}`, `start_date`, and `end_date`.\n\
                 - For any report that includes trend/technical judgment, fetch enough history to support a 200-day moving average and full technical context. In practice, request roughly 12 months of daily history or at least 260 trading bars.\n\
                 - `get_indicators` should request the indicator set needed to support the final technical view. When a full technical panel is required, you may request: close_50_sma, close_200_sma, close_10_ema, macd, macds, macdh, rsi, boll, boll_ub, boll_lb, atr, vwma, vwap, adx, kdj_k, kdj_d, kdj_j, cci, wr, obv.\n\
                 - Prefer one comprehensive indicator call instead of repeated partial calls. Do not omit indicators that the final structured technical output must show with actual values.\n\
                 - Use `{analysis_date}` as the current trading date anchor."
            ),
            "sentiment" => format!(
                "- Use `get_news` to search company-specific news and public-discussion proxies.\n\
                 - Prefer a recent window around `{analysis_date}` using `start_date` and `end_date`.\n\
                 - Search broadly enough to capture retail/investor/public-discussion proxies, but stay within the requested date window.\n\
                 - Your final report should synthesize crowd narrative, sentiment temperature, positioning, and what people appear to be reacting to.\n\
                 - If you did not retrieve direct social-post data, say that clearly and explain that sentiment is inferred from news flow, attention themes, and price behavior rather than claimed as directly observed."
            ),
            "news" => format!(
                "- Use `get_news(ticker/start_date/end_date)` for company-specific or targeted event searches.\n\
                 - Use `get_global_news(curr_date/look_back_days/limit)` for macro and broader market context.\n\
                 - Prefer recent windows ending on `{analysis_date}`.\n\
                 - You should normally fetch both company-specific and macro/global context before finalizing unless the available evidence is already clearly sufficient.\n\
                 - If `get_news` returns sparse-but-usable company evidence together with explicit data-gap metadata, you may finalize directly when the correct conclusion is precisely that company catalysts are weak or incomplete; do not force another macro fetch unless it is likely to change the thesis.\n\
                 - Separate company catalysts from macro catalysts and note second-order effects.\n\
                 - Prioritize a dated event timeline and state explicitly whether each event is bullish, bearish, or mixed for the stock."
            ),
            "fundamentals" => format!(
                "- Use `get_fundamentals` for the company overview.\n\
                 - You should normally confirm the overview with at least one statement-level tool before finalizing.\n\
                 - Use `get_income_statement` when margins, profitability, growth, or valuation interpretation depends on period consistency.\n\
                 - Use `get_cashflow` when free-cash-flow quality, capital intensity, buybacks, or cash conversion are part of the thesis.\n\
                 - Use `get_balance_sheet` when leverage, liquidity, working capital, or capital structure are part of the thesis.\n\
                 - If the overview snapshot appears internally inconsistent, period-mixed, or economically implausible, you must fetch statement-level confirmation before finalizing.\n\
                 - Use `get_insider_transactions` when governance, filing activity, insider signaling, or ownership-related events could change conviction.\n\
                 - Anchor everything to company `{symbol}` and current date `{analysis_date}`."
            ),
            _ => format!(
                "- Use the available tools only when they materially improve the evidence base for {symbol} on {analysis_date}."
            ),
        }
    }

    fn debate_directive(speaker: &str) -> &'static str {
        match speaker {
            "Bull Researcher" => {
                "You are a Bull Analyst advocating for investing in the stock. Build a strong, evidence-based case emphasizing growth potential, competitive advantages, positive indicators, and why the bearish side is overestimating the downside or missing the most important upside drivers. Engage directly with the opposing case instead of listing disconnected facts."
            }
            "Bear Researcher" => {
                "You are a Bear Analyst making the case against investing in the stock. Emphasize risks, challenges, competitive weaknesses, adverse indicators, broken assumptions, and why the bullish side is overstating upside or understating fragility. Engage directly with the opposing case instead of listing disconnected facts."
            }
            "Aggressive Analyst" => {
                "As the Aggressive Risk Analyst, actively champion high-reward and high-conviction opportunities. Focus on upside asymmetry, bold execution, and where excessive caution may leave the best opportunity on the table. Respond directly to conservative and neutral objections with data-driven rebuttals."
            }
            "Conservative Analyst" => {
                "As the Conservative Risk Analyst, prioritize capital preservation, drawdown control, and durability. Expose hidden downside, fragile assumptions, liquidity risk, and where aggressive or neutral views fail to sufficiently protect capital. Respond directly to the other viewpoints."
            }
            "Neutral Analyst" => {
                "As the Neutral Risk Analyst, challenge both extremes and build a balanced, sustainable risk framing. Weigh upside and downside, point out where aggressive views are too optimistic and conservative views too restrictive, and push toward the most robust risk-adjusted path."
            }
            _ => {
                "Write as a persuasive participant in a multi-agent trading debate. Focus on edge, rebuttal, and decision usefulness."
            }
        }
    }

    fn instrument_context(symbol: &str, market_type: &str) -> String {
        format!("Instrument: {symbol}\nMarket: {market_type}")
    }

    #[allow(dead_code)]
    fn language_label(language: &str) -> &'static str {
        match language.trim().to_lowercase().as_str() {
            "en" | "en-us" | "english" => "English",
            "zh" | "zh-cn" | "chinese" | "zhongwen" => "Simplified Chinese",
            _ => "Simplified Chinese",
        }
    }

    fn rating_scale_block() -> &'static str {
        "Rating Scale (use exactly one):
- Buy: strong conviction to enter or add aggressively
- Overweight: constructive view, add or increase exposure selectively
- Hold: evidence is balanced or edge is insufficient for action
- Underweight: cautious view, trim or stay below normal exposure
- Sell: strong conviction to exit, avoid, or actively de-risk"
    }

    fn extra_context_block(extra_context: &[(&str, &str)]) -> String {
        if extra_context.is_empty() {
            "Additional context: none.".to_string()
        } else {
            extra_context
                .iter()
                .filter_map(|(title, body)| {
                    let body = body.trim();
                    (!body.is_empty()).then(|| format!("{title}:\n{body}"))
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        }
    }

    fn bounded_context_block(
        extra_context: &[(&str, &str)],
        per_section_chars: usize,
        total_chars: usize,
    ) -> String {
        if extra_context.is_empty() {
            return "Additional context: none.".to_string();
        }

        let mut rendered = Vec::new();
        let mut used = 0usize;
        for (title, body) in extra_context {
            let body = body.trim();
            if body.is_empty() || used >= total_chars {
                continue;
            }

            let remaining = total_chars.saturating_sub(used);
            let bounded = Self::bounded_text(body, per_section_chars.min(remaining));
            if bounded.is_empty() {
                continue;
            }

            used += bounded.chars().count();
            rendered.push(format!("{title}:\n{bounded}"));
        }

        if rendered.is_empty() {
            "Additional context: none.".to_string()
        } else {
            rendered.join("\n\n")
        }
    }

}

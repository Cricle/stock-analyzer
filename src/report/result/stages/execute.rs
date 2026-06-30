use crate::task_manager::TaskRunParams;
use crate::{AnalysisResult, InvestmentDebateState, RiskDebateState};

impl crate::TaskManager {
    pub(crate) async fn run_bull_researcher_node(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        quick_llm: &crate::llm::LlmClient,
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
        let scoring_context = format!(
            "Quantitative Assessment: direction_score={}, confidence={}, action_score={}",
            result.report.direction_score,
            result.report.confidence_score,
            result.report.action_score
        );
        let bull_turn = quick_llm
            .generate_debate_turn(crate::llm::DebateTurnParams {
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
                    ("Quantitative Scoring", &scoring_context),
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
        turns.push(crate::report::graph::debate_turn_from_generated(&bull_turn));
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
        quick_llm: &crate::llm::LlmClient,
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
        let scoring_context = format!(
            "Quantitative Assessment: direction_score={}, confidence={}, action_score={}",
            result.report.direction_score,
            result.report.confidence_score,
            result.report.action_score
        );
        let bear_turn = quick_llm
            .generate_debate_turn(crate::llm::DebateTurnParams {
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
                    ("Quantitative Scoring", &scoring_context),
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
        turns.push(crate::report::graph::debate_turn_from_generated(&bear_turn));
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
            crate::report::graph::push_checkpoint(
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

    /// Run a single risk discussion round with all 3 analysts in parallel.
    pub(crate) async fn run_risk_round(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        quick_llm: &crate::llm::LlmClient,
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
                llm_a.generate_debate_turn(crate::llm::DebateTurnParams {
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
                llm_c.generate_debate_turn(crate::llm::DebateTurnParams {
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
                llm_n.generate_debate_turn(crate::llm::DebateTurnParams {
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
        turns.push(crate::report::graph::debate_turn_from_generated(
            &aggressive,
        ));
        turns.push(crate::report::graph::debate_turn_from_generated(
            &conservative,
        ));
        turns.push(crate::report::graph::debate_turn_from_generated(&neutral));

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
}

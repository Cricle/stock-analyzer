use adk_graph::{
    edge::{END, START},
    graph::{CompiledGraph, StateGraph},
    node::ExecutionConfig,
    state::State,
};
use anyhow::Context;

use crate::TaskManager;
use crate::llm::LlmClient;
use sa_models::AnalysisResult;

use crate::analysis::runtime::propagation::Propagator;
use crate::task_manager::TaskRunParams;

use super::nodes::{
    analyst_planner_node, clear_node, debate_node, portfolio_node, research_node,
    risk_discussion_node, tool_node, trader_node,
};
use super::routing::{analyst_route, debate_route};
use super::{
    CHANNEL_RESULT, NODE_BEAR, NODE_BULL, NODE_CLEAR_FUNDAMENTALS, NODE_CLEAR_MARKET,
    NODE_CLEAR_NEWS, NODE_CLEAR_SOCIAL, NODE_FUNDAMENTALS, NODE_MARKET, NODE_NEWS, NODE_PORTFOLIO,
    NODE_RESEARCH, NODE_RISK_DISCUSS, NODE_SENTIMENT, NODE_TOOLS_FUNDAMENTALS, NODE_TOOLS_MARKET,
    NODE_TOOLS_NEWS, NODE_TOOLS_SOCIAL, NODE_TRADER, analyst_node_name, clear_node_name,
    deserialize_result,
};

pub(crate) struct TradingAgentsGraph {
    pub(super) manager: TaskManager,
    pub(super) params: TaskRunParams,
    pub(super) selected_analysts: Vec<String>,
    pub(super) quick_llm: LlmClient,
    pub(super) deep_llm: LlmClient,
}

impl TradingAgentsGraph {
    pub(crate) async fn new(
        manager: &TaskManager,
        params: &TaskRunParams,
        _quote: Option<&sa_data::QuoteSnapshot>,
        _fundamentals: Option<&sa_data::FundamentalsSnapshot>,
        _news_items: &[sa_data::NewsItem],
    ) -> anyhow::Result<Self> {
        let base_llm = manager.resolve_llm_client(params).await?;
        Ok(Self {
            manager: manager.clone(),
            params: params.clone(),
            selected_analysts: TaskManager::normalized_selected_analysts(params),
            quick_llm: base_llm.with_model(params.quick_analysis_model.as_deref()),
            deep_llm: if crate::config::analysis_debug_quick_only() {
                base_llm.with_model(params.quick_analysis_model.as_deref())
            } else {
                base_llm.with_model(params.deep_analysis_model.as_deref())
            },
        })
    }

    pub(crate) async fn prepare_result(
        manager: &TaskManager,
        task: &sa_models::PersistedTask,
        params: &TaskRunParams,
    ) -> anyhow::Result<AnalysisResult> {
        Propagator::prepare_result(manager, task, params).await
    }

    #[tracing::instrument(skip_all, fields(task_id, symbol, analysis_date))]
    pub(crate) async fn execute(&self, result: &mut AnalysisResult) -> anyhow::Result<()> {
        tracing::Span::current().record("task_id", tracing::field::display(&result.task_id));
        tracing::Span::current().record("symbol", tracing::field::display(&result.symbol));
        tracing::Span::current().record(
            "analysis_date",
            tracing::field::display(&result.analysis_date),
        );
        let thread_id = crate::checkpoint::TaskCheckpointStore::thread_id(
            &result.task_id,
            &result.symbol,
            &result.analysis_date,
        );
        if result.artifacts.resumed_from_step > 0 {
            self.manager
                .checkpoint_store
                .clear_graph_runtime(&result.task_id, &result.symbol, &result.analysis_date)
                .await?;
        }
        tracing::info!(
            task_id = %result.task_id,
            symbol = %result.symbol,
            analysis_date = %result.analysis_date,
            analysts = ?self.selected_analysts,
            "starting trading graph execution"
        );
        let graph = self.compiled_graph(&result.symbol).await?;
        let mut input = State::new();
        let has_runtime_state = graph
            .get_state(&thread_id)
            .await
            .map_err(|error| anyhow::anyhow!("failed to inspect graph runtime state: {error}"))?
            .is_some_and(|state| state.contains_key(CHANNEL_RESULT));
        if !has_runtime_state {
            input.insert(
                CHANNEL_RESULT.to_string(),
                serde_json::to_value(result.clone())?,
            );
        }

        let state = graph
            .invoke(
                input,
                ExecutionConfig::new(&thread_id).with_recursion_limit(self.recursion_limit()),
            )
            .await
            .map_err(|error| anyhow::anyhow!("adk-graph execution failed: {error}"))?;

        tracing::info!(
            task_id = %result.task_id,
            symbol = %result.symbol,
            analysis_date = %result.analysis_date,
            "trading graph execution completed"
        );

        *result = deserialize_result(
            state
                .get(CHANNEL_RESULT)
                .cloned()
                .context("graph state missing analysis result")?,
        )?;
        result.artifacts.llm_token_usage = self.quick_llm.usage_summary().await;
        result.artifacts.runtime_nodes = self
            .manager
            .checkpoint_store
            .load_writes(&result.task_id, &result.symbol, &result.analysis_date)
            .await?
            .into_iter()
            .map(|write| sa_models::RuntimeNodeTrace {
                stage: write.stage,
                node: write.node,
                step: write.step,
                timestamp: write.created_at,
            })
            .collect();
        self.manager.finalize_result(result, &self.params).await?;
        Ok(())
    }

    async fn compiled_graph(&self, symbol: &str) -> anyhow::Result<CompiledGraph> {
        let max_debate_rounds = self.manager.max_debate_rounds;
        let quick_llm = self.quick_llm.clone();
        let deep_llm = self.deep_llm.clone();

        let checkpointer = self.manager.checkpoint_store.graph_checkpointer(symbol)?;

        let graph = StateGraph::with_channels(&[CHANNEL_RESULT])
            .add_node_fn(
                NODE_MARKET,
                analyst_planner_node(
                    self.manager.clone(),
                    self.params.clone(),
                    quick_llm.clone(),
                    "market",
                ),
            )
            .add_node_fn(
                NODE_SENTIMENT,
                analyst_planner_node(
                    self.manager.clone(),
                    self.params.clone(),
                    quick_llm.clone(),
                    "sentiment",
                ),
            )
            .add_node_fn(
                NODE_NEWS,
                analyst_planner_node(
                    self.manager.clone(),
                    self.params.clone(),
                    quick_llm.clone(),
                    "news",
                ),
            )
            .add_node_fn(
                NODE_FUNDAMENTALS,
                analyst_planner_node(
                    self.manager.clone(),
                    self.params.clone(),
                    quick_llm.clone(),
                    "fundamentals",
                ),
            )
            .add_node_fn(NODE_TOOLS_MARKET, tool_node(self.manager.clone(), "market"))
            .add_node_fn(
                NODE_TOOLS_SOCIAL,
                tool_node(self.manager.clone(), "sentiment"),
            )
            .add_node_fn(NODE_TOOLS_NEWS, tool_node(self.manager.clone(), "news"))
            .add_node_fn(
                NODE_TOOLS_FUNDAMENTALS,
                tool_node(self.manager.clone(), "fundamentals"),
            )
            .add_node_fn(
                NODE_CLEAR_MARKET,
                clear_node(self.manager.clone(), "market"),
            )
            .add_node_fn(
                NODE_CLEAR_SOCIAL,
                clear_node(self.manager.clone(), "sentiment"),
            )
            .add_node_fn(NODE_CLEAR_NEWS, clear_node(self.manager.clone(), "news"))
            .add_node_fn(
                NODE_CLEAR_FUNDAMENTALS,
                clear_node(self.manager.clone(), "fundamentals"),
            )
            .add_node_fn(
                NODE_BULL,
                debate_node(
                    self.manager.clone(),
                    self.params.clone(),
                    quick_llm.clone(),
                    true,
                ),
            )
            .add_node_fn(
                NODE_BEAR,
                debate_node(
                    self.manager.clone(),
                    self.params.clone(),
                    quick_llm.clone(),
                    false,
                ),
            )
            .add_node_fn(
                NODE_RESEARCH,
                research_node(
                    self.manager.clone(),
                    self.params.clone(),
                    quick_llm.clone(),
                    deep_llm.clone(),
                ),
            )
            .add_node_fn(
                NODE_TRADER,
                trader_node(self.manager.clone(), self.params.clone(), quick_llm.clone()),
            )
            .add_node_fn(
                NODE_RISK_DISCUSS,
                risk_discussion_node(self.manager.clone(), self.params.clone(), quick_llm.clone()),
            )
            .add_node_fn(
                NODE_PORTFOLIO,
                portfolio_node(self.manager.clone(), self.params.clone(), deep_llm.clone()),
            );

        // Sequential analyst chain: START → first_analyst → ... → last_analyst → bull
        let mut graph = graph;
        let first_analyst = &self.selected_analysts[0];
        graph = graph.add_edge(START, analyst_node_name(first_analyst));
        for pair in self.selected_analysts.windows(2) {
            graph = graph.add_edge(clear_node_name(&pair[0]), analyst_node_name(&pair[1]));
        }
        let last_analyst = &self.selected_analysts[self.selected_analysts.len() - 1];
        graph = graph.add_edge(clear_node_name(last_analyst), NODE_BULL);

        graph = graph
            .add_conditional_edges(
                NODE_MARKET,
                |state| analyst_route(state, "market"),
                [
                    (NODE_TOOLS_MARKET, NODE_TOOLS_MARKET),
                    (NODE_CLEAR_MARKET, NODE_CLEAR_MARKET),
                ],
            )
            .add_conditional_edges(
                NODE_SENTIMENT,
                |state| analyst_route(state, "sentiment"),
                [
                    (NODE_TOOLS_SOCIAL, NODE_TOOLS_SOCIAL),
                    (NODE_CLEAR_SOCIAL, NODE_CLEAR_SOCIAL),
                ],
            )
            .add_conditional_edges(
                NODE_NEWS,
                |state| analyst_route(state, "news"),
                [
                    (NODE_TOOLS_NEWS, NODE_TOOLS_NEWS),
                    (NODE_CLEAR_NEWS, NODE_CLEAR_NEWS),
                ],
            )
            .add_conditional_edges(
                NODE_FUNDAMENTALS,
                |state| analyst_route(state, "fundamentals"),
                [
                    (NODE_TOOLS_FUNDAMENTALS, NODE_TOOLS_FUNDAMENTALS),
                    (NODE_CLEAR_FUNDAMENTALS, NODE_CLEAR_FUNDAMENTALS),
                ],
            )
            .add_edge(NODE_TOOLS_MARKET, NODE_MARKET)
            .add_edge(NODE_TOOLS_SOCIAL, NODE_SENTIMENT)
            .add_edge(NODE_TOOLS_NEWS, NODE_NEWS)
            .add_edge(NODE_TOOLS_FUNDAMENTALS, NODE_FUNDAMENTALS)
            .add_conditional_edges(
                NODE_BULL,
                move |state| debate_route(state, max_debate_rounds),
                [(NODE_BEAR, NODE_BEAR), (NODE_RESEARCH, NODE_RESEARCH)],
            )
            .add_conditional_edges(
                NODE_BEAR,
                move |state| debate_route(state, max_debate_rounds),
                [(NODE_BULL, NODE_BULL), (NODE_RESEARCH, NODE_RESEARCH)],
            )
            .add_edge(NODE_RESEARCH, NODE_TRADER)
            .add_edge(NODE_TRADER, NODE_RISK_DISCUSS)
            .add_edge(NODE_RISK_DISCUSS, NODE_PORTFOLIO)
            .add_edge(NODE_PORTFOLIO, END);

        graph
            .compile()
            .map_err(|error| anyhow::anyhow!("failed to compile trading graph: {error}"))
            .map(|graph| graph.with_checkpointer_arc(checkpointer))
    }

    fn recursion_limit(&self) -> usize {
        if let Ok(val) = std::env::var("RECURSION_LIMIT") {
            if let Ok(n) = val.parse::<usize>() {
                return n;
            }
        }
        self.selected_analysts.len() * 6
            + self.manager.max_debate_rounds * 2
            + self.manager.max_risk_discuss_rounds * 3
            + 36
    }
}

use crate::llm::LlmClient;
use sa_models::{
    AnalysisScenarioContext, AnalysisUserContext, MemoryContextSnapshot, StockPickRequest,
};

use crate::{TaskManager, TaskRunParams};

pub async fn llm_client_for_request(
    manager: &TaskManager,
    request: &StockPickRequest,
) -> anyhow::Result<LlmClient> {
    manager
        .resolve_llm_client(&TaskRunParams {
            market_type: request.market.clone(),
            analysis_date: request
                .analysis_date
                .clone()
                .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string()),
            scenario: AnalysisScenarioContext::from_market_type(&request.market),
            selected_analysts: Vec::new(),
            past_context: String::new(),
            memory_context: MemoryContextSnapshot::default(),
            llm_base_url: None,
            llm_api_key: None,
            quick_analysis_model: None,
            deep_analysis_model: None,
            language: "zh-CN".to_string(),
            user_context: AnalysisUserContext::default(),
            user_context_prompt: String::new(),
            sector_context: String::new(),
        })
        .await
}

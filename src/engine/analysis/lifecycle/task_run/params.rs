use super::{build_user_context, build_user_context_prompt};
use crate::models::{PersistedTask, SingleAnalysisRequest};
use crate::{TaskManager, TaskRunParams};

impl TaskManager {
    pub async fn task_run_params_from_request(
        &self,
        task: &PersistedTask,
        request: &SingleAnalysisRequest,
    ) -> TaskRunParams {
        let params = request.parameters.clone().unwrap_or_default();
        let user_context = build_user_context(&params);
        let memory_context = self
            .initial_memory_context(&task.symbol, &task.market_type, None, None, &[])
            .await;
        let scenario = crate::models::AnalysisScenarioContext::from_market_type(&task.market_type);
        let sector_context = self
            .fetch_sector_context_for_analysis(&task.market_type)
            .await;
        TaskRunParams {
            market_type: task.market_type.clone(),
            analysis_date: task.analysis_date.clone(),
            scenario,
            selected_analysts: params.selected_analysts.unwrap_or_default(),
            past_context: memory_context.context_text.clone(),
            memory_context: crate::engine::task_manager::memory_snapshot_from_bundle(
                &memory_context,
            ),
            llm_base_url: params.llm_base_url,
            llm_api_key: params.llm_api_key,
            quick_analysis_model: params.quick_analysis_model,
            deep_analysis_model: params.deep_analysis_model,
            language: user_context.language.clone(),
            user_context_prompt: build_user_context_prompt(&user_context),
            user_context,
            sector_context,
        }
    }

    pub async fn mark_task_failed(
        &self,
        task_id: &str,
        error_message: String,
    ) -> anyhow::Result<()> {
        self.publish_failure(task_id, error_message).await
    }
}

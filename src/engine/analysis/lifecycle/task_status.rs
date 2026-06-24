fn task_to_summary(task: crate::models::PersistedTask) -> crate::models::AnalysisTaskSummary {
    crate::models::AnalysisTaskSummary {
        task_id: task.task_id,
        stock_code: task.symbol.clone(),
        stock_name: if task.stock_name.trim().is_empty() {
            task.symbol.clone()
        } else {
            task.stock_name.clone()
        },
        market_type: task.market_type.clone(),
        status: task.status,
        progress: task.progress,
        start_time: task.created_at.to_rfc3339(),
        created_at: task.created_at.to_rfc3339(),
        updated_at: task.updated_at.to_rfc3339(),
        llm_token_usage: task.llm_token_usage,
    }
}

use chrono::Utc;

use crate::TaskManager;
use crate::models::{AnalysisResult, ResultStage, TaskStatus, TaskStatusResponse};

impl TaskManager {
    pub async fn task_status(&self, task_id: &str) -> anyhow::Result<Option<TaskStatusResponse>> {
        let Some(task) = self.analysis_store.get_task(task_id).await? else {
            return Ok(None);
        };
        let mut result_stage = None;
        let mut report_stage_state = None;
        let mut result_data = None;
        let include_result_snapshot =
            matches!(task.status, TaskStatus::Completed | TaskStatus::Failed);
        if include_result_snapshot {
            if let Some(mut result) = self.analysis_store.get_result(task_id).await? {
                result.sync_derived_fields();
                result_stage = Some(Self::infer_result_stage(&result));
                report_stage_state = Some(result.report_stage());
                result_data = Some(result);
            }
        }
        let current_step_name = task.current_step_name.clone();
        let elapsed_anchor = match task.status {
            TaskStatus::Completed | TaskStatus::Cancelled | TaskStatus::Failed => task.updated_at,
            _ => Utc::now(),
        };
        Ok(Some(TaskStatusResponse {
            task_id: task.task_id.clone(),
            status: task.status.clone(),
            progress: task.progress,
            current_step_name,
            current_step_description: task.current_step_description,
            message: task.message,
            error_message: task.error_message,
            steps: Self::steps_for_progress(
                task.progress,
                &task.status,
                &task.current_step_name,
                result_stage.as_ref(),
                report_stage_state.as_ref(),
            ),
            elapsed_time: ((elapsed_anchor - task.created_at).num_seconds() as i32).max(0),
            remaining_time: match task.status {
                TaskStatus::Completed | TaskStatus::Cancelled | TaskStatus::Failed => 0,
                _ => (100 - task.progress).max(0),
            },
            estimated_total_time: 100,
            result_stage: match task.status {
                TaskStatus::Completed => Some(ResultStage::Complete),
                _ => result_stage,
            },
            report_stage_state,
            llm_token_usage: task.llm_token_usage.clone(),
            result_data,
            symbol: Some(task.symbol.clone()),
            stock_name: Some(if task.stock_name.trim().is_empty() {
                task.symbol.clone()
            } else {
                task.stock_name.clone()
            }),
            market_type: Some(task.market_type.clone()),
        }))
    }

    pub async fn list_tasks(
        &self,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<crate::models::AnalysisTaskSummary>> {
        let rows = self.analysis_store.list_tasks(limit, offset).await?;
        Ok(rows.into_iter().map(|task| task_to_summary(task)).collect())
    }

    pub async fn list_tasks_for_user(
        &self,
        owner_username: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<crate::models::AnalysisTaskSummary>> {
        let rows = self
            .analysis_store
            .list_tasks_for_user(owner_username, limit, offset)
            .await?;
        Ok(rows.into_iter().map(|task| task_to_summary(task)).collect())
    }

    pub async fn task_result(&self, task_id: &str) -> anyhow::Result<Option<AnalysisResult>> {
        let task = self.analysis_store.get_task(task_id).await?;
        let mut result = self.analysis_store.get_result(task_id).await?;
        if result.is_none() {
            if let Some(task_meta) = task.as_ref() {
                result = self
                    .checkpoint_store
                    .load(
                        &task_meta.task_id,
                        &task_meta.symbol,
                        &task_meta.analysis_date,
                    )
                    .await?
                    .map(|checkpoint| checkpoint.result);
            }
        }
        if let Some(ref mut result) = result {
            let is_terminal = matches!(
                task.as_ref().map(|item| &item.status),
                Some(TaskStatus::Completed) | Some(TaskStatus::Failed)
            );
            if is_terminal {
                result.sync_derived_fields();
            } else {
                let _ = self.refresh_structured_report_snapshot(result).await;
                Self::strip_incomplete_result_payload(result);
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_task(
        symbol: &str,
        stock_name: &str,
        status: TaskStatus,
    ) -> crate::models::PersistedTask {
        let now = Utc::now();
        crate::models::PersistedTask {
            task_id: "test-task-123".to_string(),
            owner_username: "user1".to_string(),
            symbol: symbol.to_string(),
            stock_name: stock_name.to_string(),
            market_type: "A-share".to_string(),
            analysis_date: "2024-01-15".to_string(),
            research_depth: "deep".to_string(),
            request: crate::models::SingleAnalysisRequest {
                symbol: Some(symbol.to_string()),
                stock_code: None,
                stock_name: Some(stock_name.to_string()),
                parameters: None,
                force_refresh: false,
            },
            status,
            progress: 50,
            current_step_name: "analyzing".to_string(),
            current_step_description: "Running analysis".to_string(),
            message: "in progress".to_string(),
            error_message: None,
            llm_token_usage: crate::models::LlmTokenUsageSummary::default(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn test_task_to_summary_basic() {
        let task = make_task("600519", "贵州茅台", TaskStatus::Running);
        let summary = task_to_summary(task);
        assert_eq!(summary.task_id, "test-task-123");
        assert_eq!(summary.stock_code, "600519");
        assert_eq!(summary.stock_name, "贵州茅台");
        assert_eq!(summary.market_type, "A-share");
        assert_eq!(summary.progress, 50);
    }

    #[test]
    fn test_task_to_summary_empty_stock_name_uses_symbol() {
        let task = make_task("600519", "", TaskStatus::Pending);
        let summary = task_to_summary(task);
        assert_eq!(summary.stock_name, "600519");
    }

    #[test]
    fn test_task_to_summary_whitespace_stock_name_uses_symbol() {
        let task = make_task("600519", "  ", TaskStatus::Completed);
        let summary = task_to_summary(task);
        assert_eq!(summary.stock_name, "600519");
    }

    #[test]
    fn test_task_to_summary_preserves_status() {
        let task = make_task("AAPL", "Apple", TaskStatus::Failed);
        let summary = task_to_summary(task);
        assert!(matches!(summary.status, TaskStatus::Failed));
    }

    #[test]
    fn test_task_to_summary_timestamps() {
        let task = make_task("AAPL", "Apple", TaskStatus::Running);
        let summary = task_to_summary(task);
        assert!(!summary.start_time.is_empty());
        assert!(!summary.created_at.is_empty());
        assert!(!summary.updated_at.is_empty());
    }
}

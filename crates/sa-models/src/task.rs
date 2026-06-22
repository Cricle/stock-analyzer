use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::analysis::{
    AnalysisResult, LlmTokenUsageSummary, ReportStageState, SingleAnalysisRequest,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::Failed => "failed",
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "running" => Ok(TaskStatus::Running),
            "completed" => Ok(TaskStatus::Completed),
            "cancelled" => Ok(TaskStatus::Cancelled),
            "failed" => Ok(TaskStatus::Failed),
            _ => Err(anyhow::anyhow!("unknown task status: {s}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisStep {
    pub name: String,
    pub description: String,
    pub status: StepStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Active,
    Success,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStatusResponse {
    pub task_id: String,
    pub status: TaskStatus,
    pub progress: i32,
    pub current_step_name: String,
    pub current_step_description: String,
    pub message: String,
    pub error_message: Option<String>,
    pub steps: Vec<AnalysisStep>,
    pub elapsed_time: i32,
    pub remaining_time: i32,
    pub estimated_total_time: i32,
    pub result_stage: Option<ResultStage>,
    pub report_stage_state: Option<ReportStageState>,
    pub llm_token_usage: LlmTokenUsageSummary,
    pub result_data: Option<AnalysisResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultStage {
    Overview,
    Analysts,
    Debate,
    Research,
    Trader,
    Risk,
    Portfolio,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskEvent {
    pub event_type: String,
    pub task_id: String,
    pub status: TaskStatus,
    pub progress: i32,
    pub message: String,
    pub current_step_name: String,
    pub current_step_description: String,
    pub emitted_at: String,
    pub result_stage: Option<ResultStage>,
    pub llm_token_usage: LlmTokenUsageSummary,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistedTask {
    pub task_id: String,
    pub owner_username: String,
    pub symbol: String,
    pub stock_name: String,
    pub market_type: String,
    pub analysis_date: String,
    pub research_depth: String,
    pub request: SingleAnalysisRequest,
    pub status: TaskStatus,
    pub progress: i32,
    pub current_step_name: String,
    pub current_step_description: String,
    pub message: String,
    pub error_message: Option<String>,
    pub llm_token_usage: LlmTokenUsageSummary,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PersistedTask {
    pub fn status_string(&self) -> &str {
        self.status.as_str()
    }
}

#[cfg(test)]
mod task_tests {
    use super::*;

    // --- TaskStatus ---

    #[test]
    fn task_status_as_str() {
        assert_eq!(TaskStatus::Pending.as_str(), "pending");
        assert_eq!(TaskStatus::Running.as_str(), "running");
        assert_eq!(TaskStatus::Completed.as_str(), "completed");
        assert_eq!(TaskStatus::Cancelled.as_str(), "cancelled");
        assert_eq!(TaskStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn task_status_from_str_valid() {
        assert_eq!(
            "pending".parse::<TaskStatus>().unwrap(),
            TaskStatus::Pending
        );
        assert_eq!(
            "running".parse::<TaskStatus>().unwrap(),
            TaskStatus::Running
        );
        assert_eq!(
            "completed".parse::<TaskStatus>().unwrap(),
            TaskStatus::Completed
        );
        assert_eq!(
            "cancelled".parse::<TaskStatus>().unwrap(),
            TaskStatus::Cancelled
        );
        assert_eq!("failed".parse::<TaskStatus>().unwrap(), TaskStatus::Failed);
    }

    #[test]
    fn task_status_from_str_invalid() {
        assert!("unknown".parse::<TaskStatus>().is_err());
    }

    #[test]
    fn task_status_roundtrip() {
        let statuses = [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Completed,
            TaskStatus::Cancelled,
            TaskStatus::Failed,
        ];
        for status in &statuses {
            let s = status.as_str();
            let restored: TaskStatus = s.parse().unwrap();
            assert_eq!(*status, restored);
        }
    }

    // --- TaskStatus serde ---

    #[test]
    fn task_status_serde_roundtrip() {
        let status = TaskStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        let restored: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, restored);
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::analysis::{
    AnalysisResult, LlmTokenUsageSummary, ReportStageState, SingleAnalysisRequest,
};

/// Lifecycle status of an analysis task.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
    BlockedData,
    BlockedLlm,
}

impl TaskStatus {
    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::Failed => "failed",
            TaskStatus::BlockedData => "blocked_data",
            TaskStatus::BlockedLlm => "blocked_llm",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed | Self::BlockedData | Self::BlockedLlm
        )
    }
}

/// Parse task status from string.
impl std::str::FromStr for TaskStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "running" => Ok(TaskStatus::Running),
            "completed" => Ok(TaskStatus::Completed),
            "cancelled" => Ok(TaskStatus::Cancelled),
            "failed" => Ok(TaskStatus::Failed),
            "blocked_data" => Ok(TaskStatus::BlockedData),
            "blocked_llm" => Ok(TaskStatus::BlockedLlm),
            _ => Err(anyhow::anyhow!("unknown task status: {s}")),
        }
    }
}

/// A single step within an analysis task pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisStep {
    pub name: String,
    pub description: String,
    pub status: StepStatus,
}

/// Status of an individual analysis pipeline step.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Active,
    Success,
    Error,
}

/// Full status snapshot of a running or completed analysis task.
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
/// Pipeline stage of a multi-stage analysis result.
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

/// SSE event emitted during task progress updates.
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

/// Analysis task persisted to the database.
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
    #[serde(default)]
    pub quality_gate_json: Option<serde_json::Value>,
    #[serde(default = "default_charge_state")]
    pub charge_state: String,
    #[serde(default)]
    pub charge_ledger_id: Option<String>,
    #[serde(default)]
    pub refund_ledger_id: Option<String>,
    #[serde(default)]
    pub retry_of_task_id: Option<String>,
    #[serde(default)]
    pub logical_request_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn default_charge_state() -> String {
    "uncharged".to_string()
}

impl PersistedTask {
    /// Get the status as a string slice.
    pub fn status_string(&self) -> &str {
        self.status.as_str()
    }
}

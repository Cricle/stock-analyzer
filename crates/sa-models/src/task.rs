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

    // --- StepStatus ---

    #[test]
    fn step_status_serde_roundtrip() {
        let statuses = [
            StepStatus::Pending,
            StepStatus::Active,
            StepStatus::Success,
            StepStatus::Error,
        ];
        for status in &statuses {
            let json = serde_json::to_string(status).unwrap();
            let restored: StepStatus = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&restored).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn step_status_json_values() {
        assert_eq!(
            serde_json::to_string(&StepStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&StepStatus::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&StepStatus::Success).unwrap(),
            "\"success\""
        );
        assert_eq!(
            serde_json::to_string(&StepStatus::Error).unwrap(),
            "\"error\""
        );
    }

    // --- ResultStage ---

    #[test]
    fn result_stage_serde_roundtrip() {
        let stages = [
            ResultStage::Overview,
            ResultStage::Analysts,
            ResultStage::Debate,
            ResultStage::Research,
            ResultStage::Trader,
            ResultStage::Risk,
            ResultStage::Portfolio,
            ResultStage::Complete,
        ];
        for stage in &stages {
            let json = serde_json::to_string(stage).unwrap();
            let restored: ResultStage = serde_json::from_str(&json).unwrap();
            assert_eq!(*stage, restored);
        }
    }

    // --- AnalysisStep ---

    #[test]
    fn analysis_step_serde_roundtrip() {
        let step = AnalysisStep {
            name: "market_analysis".to_string(),
            description: "Analyzing market data".to_string(),
            status: StepStatus::Active,
        };
        let json = serde_json::to_string(&step).unwrap();
        let restored: AnalysisStep = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "market_analysis");
        assert_eq!(restored.description, "Analyzing market data");
    }

    // --- PersistedTask ---

    #[test]
    fn persisted_task_status_string() {
        let task = PersistedTask {
            task_id: "test".to_string(),
            owner_username: "user".to_string(),
            symbol: "AAPL".to_string(),
            stock_name: "Apple".to_string(),
            market_type: "美股".to_string(),
            analysis_date: "2025-01-15".to_string(),
            research_depth: "full".to_string(),
            request: SingleAnalysisRequest {
                symbol: Some("AAPL".to_string()),
                stock_code: None,
                stock_name: Some("Apple".to_string()),
                parameters: None,
                force_refresh: false,
            },
            status: TaskStatus::Running,
            progress: 50,
            current_step_name: "market".to_string(),
            current_step_description: "Market analysis".to_string(),
            message: "Running".to_string(),
            error_message: None,
            llm_token_usage: LlmTokenUsageSummary::default(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_eq!(task.status_string(), "running");
    }

    // --- TaskEvent ---

    #[test]
    fn task_event_serde_roundtrip() {
        let event = TaskEvent {
            event_type: "progress".to_string(),
            task_id: "task-1".to_string(),
            status: TaskStatus::Running,
            progress: 30,
            message: "Processing".to_string(),
            current_step_name: "market".to_string(),
            current_step_description: "Market analysis".to_string(),
            emitted_at: "2025-01-15T00:00:00Z".to_string(),
            result_stage: Some(ResultStage::Analysts),
            llm_token_usage: LlmTokenUsageSummary::default(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: TaskEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.task_id, "task-1");
        assert_eq!(restored.status, TaskStatus::Running);
        assert_eq!(restored.progress, 30);
    }
}

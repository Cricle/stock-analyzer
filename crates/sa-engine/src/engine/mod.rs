//! sa-engine analysis modules.

pub mod config;
pub mod shared;
pub mod task_manager;
pub mod telemetry;

pub mod analysis;
pub mod memory;
pub mod checkpoint;
pub mod stock_pick;
pub mod tools;
pub mod score;
pub mod guidance;
pub mod llm;

pub use task_manager::{TaskManager, TaskRunParams};
pub use task_manager::TASK_STEPS;
pub use telemetry::{SharedTelemetry, TelemetryState};
pub use stock_pick::run as run_stock_pick;
pub use score::scorer::score_stock_pick;
pub use guidance::generate_prewarm_tasks;
pub use telemetry::{init_telemetry, record_analysis_task_duration, record_llm_usage};
pub use config::{env_flag, env_flag_value};
pub use shared::{shared_http_client, safe_ticker_component};

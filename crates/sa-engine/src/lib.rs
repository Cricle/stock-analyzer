//! sa-engine — Modular stock analysis engine.
//!
//! Users can import the whole engine or individual functions:
//!
//! ```rust,no_run
//! // Full analysis pipeline
//! use sa_engine::{TaskManager, TaskRunParams};
//!
//! // Individual modules
//! use sa_engine::stock_pick;
//! use sa_engine::score;
//! use sa_engine::guidance;
//! use sa_engine::memory;
//! use sa_engine::llm;
//! use sa_engine::qlib_import;
//! ```

// ── Core ──
pub mod config;
pub mod shared;
pub mod task_manager;
pub mod telemetry;

// ── Analysis modules ──
pub mod analysis;
pub mod memory;
pub mod checkpoint;
pub mod stock_pick;
pub mod qlib_import;
pub mod tools;
pub mod score;
pub mod guidance;
pub mod llm;

// ── Primary entry points ──
pub use task_manager::{TaskManager, TaskRunParams};
pub use task_manager::TASK_STEPS;
pub use telemetry::{SharedTelemetry, TelemetryState};

// ── Convenience re-exports ──
pub use stock_pick::run as run_stock_pick;
pub use score::scorer::score_stock_pick;
pub use qlib_import::{run_import as import_qlib, run_init_from_env as import_qlib_from_env};
pub use guidance::prewarm::generate_prewarm_tasks;
pub use guidance::embedding::{semantic_embed, hash_embed, EMBEDDING_DIMENSION};
pub use telemetry::{init_telemetry, record_analysis_task_duration, record_llm_usage};
pub use config::{env_flag, env_flag_value};
pub use shared::{shared_http_client, safe_ticker_component};

//! sa-engine — Analysis engine core.
//!
//! This crate contains the `TaskManager` which orchestrates market analysis,
//! fundamental analysis, news analysis, and research/portfolio decision steps.

pub mod config;
pub mod shared;

pub mod task_manager;
pub mod telemetry;

pub mod analysis;
pub mod checkpoint;
pub mod memory;
pub mod qlib_import;
pub mod stock_pick;

pub use task_manager::TASK_STEPS;
pub use task_manager::{TaskManager, TaskRunParams};
pub use telemetry::{SharedTelemetry, TelemetryState};

// ── Merged modules (previously separate crates) ──
pub mod guidance;
pub mod llm;
pub mod score;
pub mod tools;

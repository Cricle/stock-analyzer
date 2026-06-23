#![allow(
    clippy::collapsible_if,
    clippy::let_and_return,
    clippy::type_complexity,
    clippy::redundant_closure,
    clippy::needless_question_mark,
    clippy::unnecessary_lazy_evaluations,
    clippy::manual_contains,
    clippy::unnecessary_map_or,
    clippy::manual_clamp,
    clippy::too_many_arguments,
    clippy::unnecessary_sort_by,
    clippy::useless_conversion,
    clippy::manual_is_ascii_check,
    clippy::derivable_impls,
    clippy::redundant_field_names,
    clippy::bool_comparison,
    clippy::needless_borrow,
    clippy::if_same_then_else,
    clippy::manual_range_contains,
    clippy::should_implement_trait,
    clippy::redundant_pattern_matching
)]

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

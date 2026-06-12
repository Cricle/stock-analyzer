//! sa-engine — Monolithic stock analysis engine.

pub mod types;
pub mod models;
pub mod data;
pub mod engine;
pub mod i18n;
pub mod bin_helpers;

// Convenience re-exports at crate root
pub use engine::{
    TaskManager, TaskRunParams, TASK_STEPS,
    run_stock_pick, score_stock_pick,
    generate_prewarm_tasks,
    env_flag, env_flag_value,
    safe_ticker_component,
};
pub use engine::storage::{StorageBackend, FilesystemStorage};

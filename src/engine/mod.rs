//! sa-engine analysis modules.

pub mod config;
pub mod math_utils;
pub mod shared;
pub mod storage;
pub mod task_manager;

pub mod analysis;
pub mod memory;
pub mod checkpoint;
pub mod stock_pick;
pub mod store;
pub mod tools;
pub mod score;
pub mod guidance;
pub mod llm;

pub use task_manager::{TaskManager, TaskRunParams};
pub use task_manager::TASK_STEPS;
pub use stock_pick::run as run_stock_pick;
pub use score::scorer::score_stock_pick;
pub use guidance::generate_prewarm_tasks;
pub use config::{env_flag, env_flag_value};
pub use shared::safe_ticker_component;

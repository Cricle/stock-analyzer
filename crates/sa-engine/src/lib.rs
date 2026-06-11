//! sa-engine — Monolithic stock analysis engine.

pub mod types;
pub mod models;
pub mod data;
pub mod engine;
pub mod i18n;

// Convenience re-exports at crate root
pub use engine::{
    TaskManager, TaskRunParams, TASK_STEPS,
    SharedTelemetry, TelemetryState,
    run_stock_pick, score_stock_pick,
    import_qlib, import_qlib_from_env,
    generate_prewarm_tasks,
    semantic_embed, hash_embed, EMBEDDING_DIMENSION,
    init_telemetry, record_analysis_task_duration, record_llm_usage,
    env_flag, env_flag_value,
    shared_http_client, safe_ticker_component,
};

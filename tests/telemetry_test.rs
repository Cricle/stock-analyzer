use std::sync::Arc;

use stock_analyzer::telemetry::{
    TelemetryState, init_telemetry, mark_span_task, record_analysis_task_duration, record_llm_usage,
};

#[test]
fn telemetry_state_new() {
    let state = TelemetryState::new();
    let _ = state.meter();
}

#[test]
fn telemetry_state_default() {
    let state = TelemetryState::default();
    let _ = state.meter();
}

#[test]
fn init_telemetry_creates_shared() {
    let telemetry = init_telemetry();
    assert!(Arc::strong_count(&telemetry) >= 1);
}

#[test]
fn record_analysis_task_duration_success() {
    let state = TelemetryState::new();
    record_analysis_task_duration(&state, "completed", "US", 1500.0, None);
}

#[test]
fn record_analysis_task_duration_with_error() {
    let state = TelemetryState::new();
    record_analysis_task_duration(&state, "failed", "US", 500.0, Some("timeout"));
}

#[test]
fn record_llm_usage_success() {
    let state = TelemetryState::new();
    record_llm_usage(&state, "gpt-4", 100, 200, 300, 1000.0, true);
}

#[test]
fn record_llm_usage_failure() {
    let state = TelemetryState::new();
    record_llm_usage(&state, "gpt-4", 50, 0, 50, 500.0, false);
}

#[test]
fn mark_span_task_does_not_panic() {
    mark_span_task("task-123", "AAPL", "US");
}

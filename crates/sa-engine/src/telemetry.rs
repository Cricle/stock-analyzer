//! Telemetry types for the engine.
//!
//! `SharedTelemetry` is a lightweight handle. The full `TelemetryState`
//! with OTel counters and histograms lives in the backend crate and will
//! be migrated here in a later task.

use std::sync::Arc;

use opentelemetry::KeyValue;
use opentelemetry::metrics::{Counter, Histogram, Meter};
use tracing::{Span, field};

/// Prometheus registry + OTel meter state.
///
/// This is a stub that will be replaced by the full `TelemetryState`
/// from `backend/src/telemetry.rs` once the backend is reduced to a thin shell.
#[derive(Clone)]
pub struct TelemetryState {
    meter: Meter,
    // Analysis
    pub analysis_requests_total: Counter<u64>,
    pub analysis_task_duration_ms: Histogram<f64>,
    // LLM
    pub llm_requests_total: Counter<u64>,
    pub llm_request_duration_ms: Histogram<f64>,
    pub llm_tokens_prompt_total: Counter<u64>,
    pub llm_tokens_completion_total: Counter<u64>,
    pub llm_tokens_total: Counter<u64>,
    pub llm_errors_total: Counter<u64>,
}

impl TelemetryState {
    /// Create a new `TelemetryState` with default OTel meters.
    pub fn new() -> Self {
        let meter = opentelemetry::global::meter("tradingagents");
        Self {
            meter: meter.clone(),
            analysis_requests_total: meter.u64_counter("analysis_requests_total").build(),
            analysis_task_duration_ms: meter.f64_histogram("analysis_task_duration_ms").build(),
            llm_requests_total: meter.u64_counter("llm_requests_total").build(),
            llm_request_duration_ms: meter.f64_histogram("llm_request_duration_ms").build(),
            llm_tokens_prompt_total: meter.u64_counter("llm_tokens_prompt_total").build(),
            llm_tokens_completion_total: meter.u64_counter("llm_tokens_completion_total").build(),
            llm_tokens_total: meter.u64_counter("llm_tokens_total").build(),
            llm_errors_total: meter.u64_counter("llm_errors_total").build(),
        }
    }

    /// Borrow the underlying OTel [`Meter`].
    pub fn meter(&self) -> &Meter {
        &self.meter
    }
}

/// Shared, clone-friendly telemetry handle used across the engine.
pub type SharedTelemetry = Arc<TelemetryState>;

/// Create a default `SharedTelemetry` instance.
pub fn init_telemetry() -> SharedTelemetry {
    Arc::new(TelemetryState::new())
}

/// Record the duration and status of an analysis task.
pub fn record_analysis_task_duration(
    telemetry: &TelemetryState,
    status: &'static str,
    market_type: &str,
    elapsed_ms: f64,
    error_reason: Option<&'static str>,
) {
    let attrs = [
        KeyValue::new("analysis.status", status),
        KeyValue::new("analysis.market_type", market_type.to_string()),
        KeyValue::new("analysis.error_reason", error_reason.unwrap_or("none")),
    ];
    telemetry.analysis_requests_total.add(1, &attrs);
    telemetry
        .analysis_task_duration_ms
        .record(elapsed_ms, &attrs);
}

/// Record LLM usage metrics.
pub fn record_llm_usage(
    telemetry: &TelemetryState,
    model: &str,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    elapsed_ms: f64,
    success: bool,
) {
    let attrs = [
        KeyValue::new("llm.model", model.to_string()),
        KeyValue::new("llm.success", success.to_string()),
    ];
    telemetry.llm_requests_total.add(1, &attrs);
    telemetry.llm_request_duration_ms.record(elapsed_ms, &attrs);
    telemetry.llm_tokens_prompt_total.add(prompt_tokens, &attrs);
    telemetry
        .llm_tokens_completion_total
        .add(completion_tokens, &attrs);
    telemetry.llm_tokens_total.add(total_tokens, &attrs);
    if !success {
        telemetry.llm_errors_total.add(1, &attrs);
    }
}

/// Mark the current tracing span with task metadata.
pub fn mark_span_task(task_id: &str, symbol: &str, market_type: &str) {
    let span = Span::current();
    span.record("task_id", field::display(task_id));
    span.record("symbol", field::display(symbol));
    span.record("market_type", field::display(market_type));
}

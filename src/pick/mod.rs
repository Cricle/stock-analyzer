//! Stock pick module — candidate resolution, scoring, and LLM-based selection.

pub mod types;
pub use types::*;

mod history;
pub(crate) use history::{StockPickEvidencePayload, StockPickHistoryStore};

pub mod pipeline;
pub use pipeline::run;

pub mod scoring;
pub(crate) use scoring::{
    apply_portfolio_constraints, enrich_candidates, infer_theme_key, score_candidates,
};

pub mod objective;

mod llm_utils;
pub use llm_utils::llm_client_for_request;

pub mod validation;
pub use validation::{PickQualityGate, PickValidation, apply_defaults, validate_pick};

pub mod provenance;
pub use provenance::{DataProvenance, ProvenanceSnapshot};

pub mod gates;
pub use gates::{QualityGateRejection, apply_quality_gates};

pub mod reasoning;
pub use reasoning::{ReasoningConsistencyCheck, ReasoningConsistencyReport, validate_reasoning_consistency};

pub mod completeness;
pub use completeness::{CriticalFieldCompleteness, score_critical_field_completeness};

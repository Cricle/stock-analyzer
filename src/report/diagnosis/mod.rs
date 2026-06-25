//! Post-LLM diagnosis and auto-fix pipeline.
//!
//! Runs after all LLM stages complete and before the result is persisted.
//! Detects common quality issues in `AnalysisResult` and applies in-place
//! corrections, logging each fix as a `DiagnosisIssue`.

pub mod consistency;

pub use consistency::ConsistencyValidator;

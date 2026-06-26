//! Tool-based data collection for LLM analysis results.
//!
//! Instead of parsing LLM output text, the LLM calls tools to set data.
//! This is thread-safe for parallel reports and multi-user scenarios.

mod collector;
mod schema;
mod tools;

pub use collector::AnalysisDataCollector;
pub use schema::*;
pub use tools::*;

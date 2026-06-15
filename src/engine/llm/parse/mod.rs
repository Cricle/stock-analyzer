use anyhow::bail;
use serde_json::Value;

use super::{
    GeneratedAnalystDecision, GeneratedDebateTurn, GeneratedPortfolioDecision,
    GeneratedResearchManager, GeneratedRoleReport,
    GeneratedTraderDecision,
};

pub mod diagnosis;
pub use diagnosis::{DiagnosisIssue, IssueSeverity};

include!("parsers.rs");
include!("candidates.rs");
include!("json_extract.rs");
include!("json_string.rs");
include!("helpers.rs");
include!("validate.rs");
include!("tests.rs");

use anyhow::Context;

use super::{
    GeneratedAnalystDecision, GeneratedDebateTurn, GeneratedPortfolioDecision,
    GeneratedResearchManager, GeneratedTraderDecision, LlmClient, parse,
};

include!("generate.rs");
include!("templates.rs");
include!("prompts.rs");
include!("calibration.rs");

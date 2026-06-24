use super::{
    GeneratedAnalystDecision, GeneratedDebateTurn, GeneratedPortfolioDecision,
    GeneratedResearchManager, GeneratedRoleReport, GeneratedSubscriptionQaAnswer,
    GeneratedTraderDecision,
};

pub mod diagnosis;
pub use diagnosis::{DiagnosisIssue, IssueSeverity};

pub(crate) mod parsers;
pub(crate) mod candidates;
pub(crate) mod json_extract;
pub(crate) mod json_string;
pub(crate) mod parse_utils;
pub(crate) mod validate;
pub(crate) mod tests;

pub(crate) use parsers::{parse_generated_debate_turn, parse_generated_research_manager};
pub(crate) use parse_utils::{normalize_value, normalize_probability, text_or_default, string_list_or_default, normalize_probability_triplet};
pub(crate) use validate::validate_analyst_decision;
pub(crate) use validate::validate_research_manager;
pub(crate) use candidates::parse_object_candidates_value;
pub(crate) use json_extract::{strip_code_fence, slice_outer_json_object, slice_first_complete_json_value, repair_common_malformed_json_variants};
pub(crate) use json_string::{extract_simple_json_string_field, extract_relaxed_json_string_field, extract_json_value_before_known_field};
pub(crate) use validate::validate_trader_decision;
pub(crate) use parsers::{parse_generated_trader_decision, parse_generated_subscription_qa_answer, parse_generated_portfolio_decision, parse_generated_analyst_decision};
pub(crate) use json_string::{skip_json_whitespace, find_json_string_end, find_json_value_end, decode_json_string_literal};

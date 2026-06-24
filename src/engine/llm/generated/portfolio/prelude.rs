use serde_json::Value;

use super::super::parse;
use super::generated_impls::{
    extract_numbered_trigger_lines, extract_object_string_list, extract_object_value,
    extract_price_target_from_texts, extract_stop_loss_from_texts, extract_time_horizon_from_texts,
    meaningful_value, object_value,
};
use super::generated_types::{
    GeneratedMissingEvidenceLadder, GeneratedPortfolioDecision, GeneratedReflection, GeneratedScenarioPath,
};

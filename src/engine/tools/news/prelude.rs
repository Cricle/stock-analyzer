use anyhow::Context;
use serde_json::{Value, json};

use crate::models::AnalysisScenarioData;
use crate::types::NewsItem;

use super::{ToolExecutionResult, TradingToolbox};

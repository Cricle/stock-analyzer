use anyhow::Context;
use serde_json::{Value, json};

use sa_models::AnalysisScenarioData;
use sa_types::NewsItem;

use super::{ToolExecutionResult, TradingToolbox};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolDefinition {
    pub fn new(name: &str, description: &str, parameters: serde_json::Value) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: name.to_string(),
                description: description.to_string(),
                parameters,
            },
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScenarioPathData {
    pub key: String,
    pub name: String,
    pub trigger: String,
    pub action: String,
    pub risk_boundary: String,
    pub position_sizing: String,
    pub stop_level: String,
    pub entry_price: Option<f64>,
    pub target: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReflectionData {
    pub strongest_part: String,
    pub weakest_uncertainty: String,
    pub next_lesson: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeStopData {
    pub deadline: String,
    pub reason: String,
}

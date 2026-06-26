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

fn string_param(description: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "value": { "type": "string", "description": description }
        },
        "required": ["value"]
    })
}

fn number_param(description: &str, min: Option<f64>, max: Option<f64>) -> serde_json::Value {
    let mut props = serde_json::json!({
        "type": "number",
        "description": description
    });
    if let Some(m) = min { props["minimum"] = serde_json::json!(m); }
    if let Some(m) = max { props["maximum"] = serde_json::json!(m); }
    serde_json::json!({
        "type": "object",
        "properties": { "value": props },
        "required": ["value"]
    })
}

/// All tool definitions for LLM analysis data collection.
pub fn analysis_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        // Rating & Confidence
        ToolDefinition::new("set_rating", "Set investment rating: Buy, Overweight, Hold, Underweight, Sell",
            serde_json::json!({"type":"object","properties":{"rating":{"type":"string","enum":["Buy","Overweight","Hold","Underweight","Sell"]}},"required":["rating"]})),
        ToolDefinition::new("set_confidence", "Set confidence score (0-100).",
            number_param("Confidence score", Some(0.0), Some(100.0))),
        ToolDefinition::new("set_action", "Set recommended action.",
            string_param("Action (buy, sell, hold, watch, accumulate, reduce)")),
        // Prices
        ToolDefinition::new("set_entry_price", "Set entry price.",
            number_param("Entry price", Some(0.0), None)),
        ToolDefinition::new("set_stop_loss", "Set stop-loss price.",
            number_param("Stop-loss price", Some(0.0), None)),
        ToolDefinition::new("set_target_price", "Set price target.",
            number_param("Target price", Some(0.0), None)),
        ToolDefinition::new("set_confirmation_level", "Set confirmation level price.",
            number_param("Confirmation level", Some(0.0), None)),
        ToolDefinition::new("set_invalidation_level", "Set invalidation level price.",
            number_param("Invalidation level", Some(0.0), None)),
        ToolDefinition::new("set_risk_reward_ratio", "Set risk-reward ratio.",
            number_param("Risk-reward ratio", Some(0.0), None)),
        // Text fields
        ToolDefinition::new("set_executive_summary", "Set executive summary (1-3 sentences).",
            string_param("Executive summary")),
        ToolDefinition::new("set_investment_thesis", "Set investment thesis.",
            string_param("Investment thesis")),
        ToolDefinition::new("set_rationale", "Set decision rationale.",
            string_param("Rationale")),
        ToolDefinition::new("set_risk_assessment", "Set risk assessment.",
            string_param("Risk assessment")),
        ToolDefinition::new("set_summary", "Set PM-level summary.",
            string_param("Summary")),
        ToolDefinition::new("set_detail", "Set detailed analysis.",
            string_param("Detail")),
        ToolDefinition::new("set_strategic_actions", "Set strategic actions.",
            string_param("Strategic actions")),
        ToolDefinition::new("set_trader_plan", "Set trader plan.",
            string_param("Trader plan")),
        // Evidence & Lists
        ToolDefinition::new("add_evidence_point", "Add evidence point.",
            string_param("Evidence point")),
        ToolDefinition::new("add_key_risk", "Add key risk.",
            string_param("Key risk")),
        ToolDefinition::new("add_trigger", "Add trigger condition.",
            string_param("Trigger")),
        ToolDefinition::new("add_next_step", "Add next step.",
            string_param("Next step")),
        ToolDefinition::new("add_blocking_gap", "Add blocking evidence gap.",
            string_param("Blocking gap")),
        ToolDefinition::new("add_tolerable_gap", "Add tolerable context gap.",
            string_param("Tolerable gap")),
        ToolDefinition::new("add_manageable_gap", "Add manageable gap.",
            string_param("Manageable gap")),
        ToolDefinition::new("add_key_number", "Add key number/metric.",
            string_param("Key number")),
        ToolDefinition::new("add_reference", "Add reference/source.",
            string_param("Reference")),
        // Probability & Scores
        ToolDefinition::new("set_probability", "Set up/down/sideways probability distribution.",
            serde_json::json!({"type":"object","properties":{
                "up":{"type":"number","minimum":0.0,"maximum":1.0},
                "down":{"type":"number","minimum":0.0,"maximum":1.0},
                "sideways":{"type":"number","minimum":0.0,"maximum":1.0}
            },"required":["up","down","sideways"]})),
        ToolDefinition::new("set_score", "Set dimension score (0-100).",
            serde_json::json!({"type":"object","properties":{
                "dimension":{"type":"string"},
                "score":{"type":"number","minimum":0,"maximum":100}
            },"required":["dimension","score"]})),
        // Scenarios
        ToolDefinition::new("add_scenario_path", "Add execution scenario path.",
            serde_json::json!({"type":"object","properties":{
                "key":{"type":"string"},"name":{"type":"string"},
                "trigger":{"type":"string"},"action":{"type":"string"},
                "risk_boundary":{"type":"string"},"position_sizing":{"type":"string"},
                "stop_level":{"type":"string"},
                "entry_price":{"type":"number"},"target":{"type":"number"}
            },"required":["key","name","action"]})),
        ToolDefinition::new("set_time_horizon", "Set time horizon.",
            string_param("Time horizon (e.g. '2-6 weeks')")),
        ToolDefinition::new("set_position_sizing", "Set position sizing.",
            string_param("Position sizing")),
        ToolDefinition::new("set_time_stop", "Set time-based exit rule.",
            serde_json::json!({"type":"object","properties":{
                "deadline":{"type":"string"},"reason":{"type":"string"}
            },"required":["deadline","reason"]})),
        // Meta
        ToolDefinition::new("set_reflection", "Set reflection/lesson learned.",
            serde_json::json!({"type":"object","properties":{
                "strongest_part":{"type":"string"},
                "weakest_uncertainty":{"type":"string"},
                "next_lesson":{"type":"string"}
            },"required":["strongest_part","weakest_uncertainty","next_lesson"]})),
        ToolDefinition::new("set_accounting_scope_hypothesis", "Set accounting scope hypothesis.",
            string_param("Accounting scope hypothesis")),
        ToolDefinition::new("set_speaker", "Set speaker name for debate turn.",
            string_param("Speaker name")),
        ToolDefinition::new("set_stance", "Set debate stance (bull, bear, neutral).",
            string_param("Stance")),
        ToolDefinition::new("set_response", "Set debate response content.",
            string_param("Debate response")),
    ]
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

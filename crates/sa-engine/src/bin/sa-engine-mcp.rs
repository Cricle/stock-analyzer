//! sa-engine MCP server — stock analysis over Model Context Protocol.

use std::sync::Arc;

use clap::Parser;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolResult, Content, Implementation, ListToolsResult, PaginatedRequestParam,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::Error as McpError;

use sa_engine::bin_helpers;
use sa_engine::data::MarketDataClient;
use sa_engine::engine::guidance::{DailyGuidanceGenerator, DailyGuidanceRequest};
use sa_engine::engine::llm::LlmClient;
use sa_engine::engine::stock_pick;
use sa_engine::models::StockPickRequest;

#[derive(Parser)]
#[command(name = "sa-engine-mcp", about = "Stock Analyzer MCP server")]
struct Cli {
    #[arg(long, default_value = "stdio")]
    transport: String,
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[derive(Clone)]
struct StockAnalyzerServer {
    market_data: MarketDataClient,
    llm: LlmClient,
}

fn make_schema(
    properties: serde_json::Map<String, serde_json::Value>,
    required: &[&str],
) -> rmcp::model::JsonObject {
    let required_values: Vec<serde_json::Value> = required
        .iter()
        .map(|s| serde_json::Value::String(s.to_string()))
        .collect();
    let mut schema = serde_json::Map::new();
    schema.insert("type".into(), serde_json::json!("object"));
    schema.insert("properties".into(), serde_json::Value::Object(properties));
    if !required_values.is_empty() {
        schema.insert("required".into(), serde_json::json!(required_values));
    }
    schema
}

fn success_content(value: &serde_json::Value) -> Vec<Content> {
    vec![Content::text(serde_json::to_string_pretty(value).unwrap())]
}

fn error_content(code: &str, message: &str) -> Vec<Content> {
    let payload = serde_json::json!({"error": {"code": code, "message": message}});
    vec![Content::text(serde_json::to_string(&payload).unwrap())]
}

fn tool_generate_guidance() -> Tool {
    let mut props = serde_json::Map::new();
    props.insert(
        "market".into(),
        serde_json::json!({"type": "string", "default": "a-share", "description": "Target market"}),
    );
    Tool::new("generate_guidance", "Generate daily market guidance", Arc::new(make_schema(props, &[])))
}

fn tool_stock_pick() -> Tool {
    let mut props = serde_json::Map::new();
    props.insert("market".into(), serde_json::json!({"type": "string", "default": "a-share"}));
    props.insert("date".into(), serde_json::json!({"type": "string", "description": "YYYY-MM-DD"}));
    Tool::new("stock_pick", "Pick stocks for a market", Arc::new(make_schema(props, &[])))
}

fn tool_generate_report() -> Tool {
    let mut props = serde_json::Map::new();
    props.insert("symbol".into(), serde_json::json!({"type": "string", "description": "Stock symbol"}));
    props.insert("market".into(), serde_json::json!({"type": "string", "description": "Market (optional)"}));
    props.insert("sections".into(), serde_json::json!({"type": "array", "items": {"type": "string"}}));
    Tool::new("generate_report", "Generate analysis report for a stock", Arc::new(make_schema(props, &["symbol"])))
}

impl ServerHandler for StockAnalyzerServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "sa-engine-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some("Stock Analyzer MCP server. Tools: generate_guidance, stock_pick, generate_report".into()),
        }
    }

    async fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            next_cursor: None,
            tools: vec![tool_generate_guidance(), tool_stock_pick(), tool_generate_report()],
        })
    }

    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let args = request.arguments.unwrap_or_default();
        let name: &str = &request.name;

        match name {
            "generate_guidance" => {
                let market = args.get("market").and_then(|v| v.as_str()).unwrap_or("a-share");
                let memory = bin_helpers::build_memory();
                let generator = DailyGuidanceGenerator::new(self.market_data.clone(), memory)
                    .with_llm(self.llm.clone());
                let req = DailyGuidanceRequest {
                    market: Some(market.to_string()),
                    tickers: None,
                    refresh: None,
                };
                match generator.generate(&req).await {
                    Ok(report) => Ok(CallToolResult::success(success_content(&serde_json::json!(report)))),
                    Err(e) => Ok(CallToolResult::success(error_content("guidance_failed", &e.to_string()))),
                }
            }
            "stock_pick" => {
                let market = args.get("market").and_then(|v| v.as_str()).unwrap_or("a-share");
                let date = args.get("date").and_then(|v| v.as_str()).map(String::from);
                let req = StockPickRequest {
                    market: market.to_string(),
                    analysis_date: date,
                    language: Some("zh-CN".to_string()),
                    strategy: None,
                    candidate_symbols: None,
                    sector_type: None,
                    candidate_limit: None,
                    pick_count: None,
                    target_output_mode: None,
                    search_depth: None,
                    history_retrieval: None,
                };
                match stock_pick::run(&self.market_data, &self.llm, &req).await {
                    Ok(response) => Ok(CallToolResult::success(success_content(&serde_json::json!(response)))),
                    Err(e) => Ok(CallToolResult::success(error_content("stock_pick_failed", &e.to_string()))),
                }
            }
            "generate_report" => {
                let symbol = match args.get("symbol").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Ok(CallToolResult::success(error_content("missing_param", "symbol is required"))),
                };
                let market = args.get("market").and_then(|v| v.as_str()).unwrap_or("a-share");

                let memory = bin_helpers::build_memory();
                let generator = DailyGuidanceGenerator::new(self.market_data.clone(), memory)
                    .with_llm(self.llm.clone());
                let guidance_req = DailyGuidanceRequest {
                    market: Some(market.to_string()),
                    tickers: Some(vec![symbol.clone()]),
                    refresh: None,
                };

                let mut result = serde_json::json!({"symbol": symbol, "market": market});

                match generator.generate(&guidance_req).await {
                    Ok(report) => result["guidance"] = serde_json::json!(report),
                    Err(e) => result["guidance_error"] = serde_json::json!(e.to_string()),
                }

                let pick_req = StockPickRequest {
                    market: market.to_string(),
                    candidate_symbols: Some(vec![symbol.clone()]),
                    pick_count: Some(1),
                    language: Some("zh-CN".to_string()),
                    strategy: None,
                    sector_type: None,
                    candidate_limit: None,
                    analysis_date: None,
                    target_output_mode: None,
                    search_depth: None,
                    history_retrieval: None,
                };

                match stock_pick::run(&self.market_data, &self.llm, &pick_req).await {
                    Ok(response) => result["analysis"] = serde_json::json!(response),
                    Err(e) => result["analysis_error"] = serde_json::json!(e.to_string()),
                }

                if let Some(sections) = args.get("sections").and_then(|v| v.as_array()) {
                    result["requested_sections"] = serde_json::json!(sections);
                }

                Ok(CallToolResult::success(success_content(&result)))
            }
            _ => Err(McpError::invalid_params(format!("unknown tool: {name}"), None)),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let market_data = bin_helpers::build_market_data_client().await?;
    let llm = bin_helpers::build_llm_client()?;

    let server = StockAnalyzerServer { market_data, llm };

    match cli.transport.as_str() {
        "stdio" => {
            use rmcp::ServiceExt;
            let transport = rmcp::transport::stdio();
            let running = server.serve(transport).await?;
            running.waiting().await?;
        }
        "http" => {
            eprintln!("HTTP+SSE transport on port {} is not yet implemented", cli.port);
            std::process::exit(1);
        }
        other => {
            eprintln!("unknown transport: {other}");
            std::process::exit(1);
        }
    }

    Ok(())
}

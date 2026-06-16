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
    /// Output compact JSON in tool responses instead of pretty-printed.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Clone)]
struct StockAnalyzerServer {
    market_data: MarketDataClient,
    llm: LlmClient,
    compact_json: bool,
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

fn success_content(value: &serde_json::Value, compact: bool) -> Vec<Content> {
    let text = if compact {
        serde_json::to_string(value).unwrap()
    } else {
        serde_json::to_string_pretty(value).unwrap()
    };
    vec![Content::text(text)]
}

fn error_content(code: &str, message: &str) -> Vec<Content> {
    let payload = serde_json::json!({"error": {"code": code, "message": message}});
    vec![Content::text(serde_json::to_string(&payload).unwrap())]
}

fn tool_generate_guidance() -> Tool {
    let mut props = serde_json::Map::new();
    props.insert("market".into(), serde_json::json!({"type": "string", "default": "a-share", "description": "Market: a-share, hk, us"}));
    props.insert("lang".into(), serde_json::json!({"type": "string", "default": "zh", "description": "Output language: zh or en"}));
    Tool::new("generate_guidance", "Generate daily market guidance: sentiment, sector highlights, risk alerts, key news.", Arc::new(make_schema(props, &[])))
}

fn tool_stock_pick() -> Tool {
    let mut props = serde_json::Map::new();
    props.insert("market".into(), serde_json::json!({"type": "string", "default": "a-share", "description": "Market: a-share, hk, us"}));
    props.insert("lang".into(), serde_json::json!({"type": "string", "default": "zh", "description": "Output language: zh or en"}));
    props.insert("date".into(), serde_json::json!({"type": "string", "description": "Analysis date YYYY-MM-DD (default: today)"}));
    props.insert("candidate_symbols".into(), serde_json::json!({"type": "array", "items": {"type": "string"}, "description": "Explicit stock symbols to evaluate"}));
    Tool::new("stock_pick", "Run multi-factor stock selection with LLM analysis. Returns ranked picks with score, thesis, catalysts, and risks.", Arc::new(make_schema(props, &[])))
}

fn tool_generate_report() -> Tool {
    let mut props = serde_json::Map::new();
    props.insert("symbol".into(), serde_json::json!({"type": "string", "description": "Stock symbol (e.g. 600519.SH, 00700.HK, AAPL)"}));
    props.insert("market".into(), serde_json::json!({"type": "string", "description": "Market: a-share, hk, us"}));
    props.insert("lang".into(), serde_json::json!({"type": "string", "default": "zh", "description": "Output language: zh or en"}));
    props.insert("sections".into(), serde_json::json!({"type": "array", "items": {"type": "string"}, "description": "Report sections to include"}));
    Tool::new("generate_report", "Generate per-symbol analysis report combining guidance context and stock pick evaluation.", Arc::new(make_schema(props, &["symbol"])))
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
            instructions: Some("Stock analysis engine. generate_guidance: daily market overview. stock_pick: multi-factor stock selection. report: per-symbol analysis. All tools accept market (a-share/hk/us) and lang (zh/en).".into()),
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
                let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("zh");
                let memory = bin_helpers::build_memory();
                let generator = DailyGuidanceGenerator::new(self.market_data.clone(), memory)
                    .with_llm(self.llm.clone());
                let req = DailyGuidanceRequest {
                    market: Some(market.to_string()),
                    tickers: None,
                    refresh: None,
                    lang: Some(lang.to_string()),
                };
                match generator.generate(&req).await {
                    Ok(report) => {
                        let mut out = serde_json::json!(report);
                        let i18n = sa_engine::i18n::I18n::new();
                        out = bin_helpers::resolve_output(out, &i18n, lang);
                        Ok(CallToolResult::success(success_content(&out, self.compact_json)))
                    },
                    Err(e) => Ok(CallToolResult::success(error_content("guidance_failed", &e.to_string()))),
                }
            }
            "stock_pick" => {
                let market = args.get("market").and_then(|v| v.as_str()).unwrap_or("a-share");
                let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("zh");
                let date = args.get("date").and_then(|v| v.as_str()).map(String::from);
                let req = StockPickRequest {
                    market: market.to_string(),
                    analysis_date: date,
                    language: Some(lang.to_string()),
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
                    Ok(response) => {
                        let mut out = serde_json::json!(response);
                        let i18n = sa_engine::i18n::I18n::new();
                        out = bin_helpers::resolve_output(out, &i18n, lang);
                        Ok(CallToolResult::success(success_content(&out, self.compact_json)))
                    },
                    Err(e) => Ok(CallToolResult::success(error_content("stock_pick_failed", &e.to_string()))),
                }
            }
            "generate_report" => {
                let symbol = match args.get("symbol").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Ok(CallToolResult::success(error_content("missing_param", "symbol is required"))),
                };
                let market = args.get("market").and_then(|v| v.as_str()).unwrap_or("a-share");
                let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("zh");

                let memory = bin_helpers::build_memory();
                let generator = DailyGuidanceGenerator::new(self.market_data.clone(), memory)
                    .with_llm(self.llm.clone());
                let guidance_req = DailyGuidanceRequest {
                    market: Some(market.to_string()),
                    tickers: Some(vec![symbol.clone()]),
                    refresh: None,
                    lang: Some(lang.to_string()),
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
                    language: Some(lang.to_string()),
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

                let i18n = sa_engine::i18n::I18n::new();
                let resolved = bin_helpers::resolve_output(result, &i18n, lang);
                Ok(CallToolResult::success(success_content(&resolved, self.compact_json)))
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

    let server = StockAnalyzerServer { market_data, llm, compact_json: cli.json };

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

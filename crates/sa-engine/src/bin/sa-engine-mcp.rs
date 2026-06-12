use std::sync::Arc;

use clap::Parser;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolResult, Content, Implementation, ListToolsResult, PaginatedRequestParam,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::Error as McpError;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "sa-engine-mcp", about = "Stock Analyzer MCP server")]
struct Cli {
    /// Transport mode: "stdio" (default) or "http"
    #[arg(long, default_value = "stdio")]
    transport: String,

    /// Port for HTTP transport
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct StockAnalyzerServer;

// -- helpers ----------------------------------------------------------------

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

fn stub_response(tool: &str, i18n_suffix: &str) -> Vec<Content> {
    let payload = serde_json::json!({
        "data": {},
        "i18n_keys": [format!("{}.{}", tool, i18n_suffix)],
        "lang": "zh",
    });
    vec![Content::text(serde_json::to_string(&payload).unwrap())]
}

// -- tool definitions -------------------------------------------------------

fn tool_generate_guidance() -> Tool {
    let mut props = serde_json::Map::new();
    props.insert(
        "market".into(),
        serde_json::json!({
            "type": "string",
            "default": "a-share",
            "description": "Target market identifier",
        }),
    );
    Tool::new(
        "generate_guidance",
        "Generate market guidance",
        Arc::new(make_schema(props, &[])),
    )
}

fn tool_stock_pick() -> Tool {
    let mut props = serde_json::Map::new();
    props.insert(
        "market".into(),
        serde_json::json!({
            "type": "string",
            "default": "a-share",
            "description": "Target market identifier",
        }),
    );
    props.insert(
        "date".into(),
        serde_json::json!({
            "type": "string",
            "description": "Date in YYYY-MM-DD format (optional, defaults to today)",
        }),
    );
    Tool::new(
        "stock_pick",
        "Pick stocks for a given market and date",
        Arc::new(make_schema(props, &[])),
    )
}

fn tool_generate_report() -> Tool {
    let mut props = serde_json::Map::new();
    props.insert(
        "symbol".into(),
        serde_json::json!({
            "type": "string",
            "description": "Stock symbol, e.g. '600519'",
        }),
    );
    props.insert(
        "market".into(),
        serde_json::json!({
            "type": "string",
            "description": "Target market identifier (optional)",
        }),
    );
    props.insert(
        "sections".into(),
        serde_json::json!({
            "type": "array",
            "items": {"type": "string"},
            "description": "Report sections to include (optional)",
        }),
    );
    Tool::new(
        "generate_report",
        "Generate an analysis report for a stock",
        Arc::new(make_schema(props, &["symbol"])),
    )
}

// -- ServerHandler implementation -------------------------------------------

impl ServerHandler for StockAnalyzerServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "sa-engine-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some("Stock Analyzer MCP server".into()),
        }
    }

    async fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            next_cursor: None,
            tools: vec![
                tool_generate_guidance(),
                tool_stock_pick(),
                tool_generate_report(),
            ],
        })
    }

    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name: &str = &request.name;
        match name {
            "generate_guidance" => Ok(CallToolResult::success(stub_response(
                "guidance",
                "title",
            ))),
            "stock_pick" => Ok(CallToolResult::success(stub_response(
                "stock_pick",
                "title",
            ))),
            "generate_report" => Ok(CallToolResult::success(stub_response(
                "report",
                "title",
            ))),
            _ => Err(McpError::invalid_params(
                format!("unknown tool: {name}"),
                None,
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.transport.as_str() {
        "stdio" => {
            use rmcp::ServiceExt;
            let server = StockAnalyzerServer::default();
            let transport = rmcp::transport::stdio();
            let running = server.serve(transport).await?;
            running.waiting().await?;
        }
        "http" => {
            eprintln!(
                "HTTP+SSE transport on port {} is not yet implemented",
                cli.port
            );
            std::process::exit(1);
        }
        other => {
            eprintln!("unknown transport: {other}");
            std::process::exit(1);
        }
    }

    Ok(())
}

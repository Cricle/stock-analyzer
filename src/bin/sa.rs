//! sa — stock analysis CLI & MCP server (unified binary).

use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

use stock_analyser::bin_helpers;
use stock_analyser::data::MarketDataClient;
use stock_analyser::engine::guidance::{DailyGuidanceGenerator, DailyGuidanceRequest};
use stock_analyser::engine::llm::LlmClient;
use stock_analyser::engine::stock_pick;
use stock_analyser::i18n::I18n;
use stock_analyser::models::StockPickRequest;

// ── CLI ─────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "sa", version, about = "Stock analysis engine — CLI & MCP server")]
struct Cli {
    /// Output compact JSON (single line) instead of pretty-printed.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate daily market guidance: sentiment, sectors, risks, key news.
    Guidance {
        #[arg(long, value_enum, default_value_t = MarketArg::AShare)]
        market: MarketArg,
        #[arg(long, value_enum)]
        lang: Option<LangArg>,
    },

    /// Run multi-factor stock selection with LLM analysis.
    StockPick {
        #[arg(long, value_enum, default_value_t = MarketArg::AShare)]
        market: MarketArg,
        #[arg(long, help = "Analysis date (YYYY-MM-DD, default: today)")]
        date: Option<String>,
        #[arg(long, value_delimiter = ',')]
        candidate_symbols: Option<Vec<String>>,
        #[arg(long, help = "Sector type for candidate search (required for HK/US)")]
        sector_type: Option<String>,
        #[arg(long, value_enum)]
        lang: Option<LangArg>,
    },

    /// Generate per-symbol analysis report.
    Report {
        #[arg(long, help = "Stock symbol (e.g. 600519.SH, 00700.HK, AAPL)")]
        symbol: String,
        #[arg(long, value_enum, default_value_t = MarketArg::AShare)]
        market: MarketArg,
        #[arg(long, value_delimiter = ',')]
        sections: Option<Vec<String>>,
        #[arg(long, value_enum)]
        lang: Option<LangArg>,
    },

    /// Start MCP server (stdio or HTTP+SSE).
    Mcp {
        #[arg(long, default_value = "stdio", help = "Transport: stdio | http")]
        transport: String,
        #[arg(long, default_value_t = 3000)]
        port: u16,
        /// Path to config file (default: ~/.config/sa-engine/config.toml or SA_ENGINE_CONFIG).
        #[arg(long)]
        config: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum MarketArg {
    AShare,
    #[value(alias = "hk")]
    Hk,
    Us,
}

impl MarketArg {
    fn as_str(self) -> &'static str {
        match self {
            MarketArg::AShare => "a-share",
            MarketArg::Hk => "hk",
            MarketArg::Us => "us",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LangArg {
    Zh,
    En,
}

impl LangArg {
    fn as_str(self) -> &'static str {
        match self { LangArg::Zh => "zh", LangArg::En => "en" }
    }
    fn as_llm_lang(self) -> &'static str {
        match self { LangArg::Zh => "zh-CN", LangArg::En => "en" }
    }
}

fn error_exit(code: &str, message: &str) -> ! {
    eprintln!(
        "{}",
        serde_json::to_string(&json!({"error": {"code": code, "message": message}})).unwrap()
    );
    std::process::exit(1);
}

fn print_json(value: &serde_json::Value, compact: bool) {
    if compact {
        println!("{}", serde_json::to_string(value).unwrap());
    } else {
        println!("{}", serde_json::to_string_pretty(value).unwrap());
    }
}

// ── Config ──────────────────────────────────────────────────────────────────

/// Load MCP key from env var or config file.
/// Priority: SA_MCP_KEY env > config file [api_keys].mcp_key or top-level mcp_key.
fn load_mcp_key(config_path: Option<&str>) -> Option<String> {
    // Env var takes priority.
    if let Ok(key) = std::env::var("SA_MCP_KEY") && !key.is_empty() {
        return Some(key);
    }
    let path = config_path
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("SA_ENGINE_CONFIG").ok().map(std::path::PathBuf::from)
        })
        .or_else(|| {
            std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join(".config/sa-engine/config.toml"))
        })?;
    let content = std::fs::read_to_string(&path).ok()?;
    let table: toml::Table = toml::from_str(&content).ok()?;
    // Check [api_keys].mcp_key first, then top-level mcp_key.
    let key = table
        .get("api_keys").and_then(|t| t.get("mcp_key")).and_then(|v| v.as_str())
        .or_else(|| table.get("mcp_key").and_then(|v| v.as_str()))?;
    if key.is_empty() { None } else { Some(key.to_string()) }
}

// ── MCP Server ──────────────────────────────────────────────────────────────

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolResult, Content, Implementation, ListToolsResult, PaginatedRequestParam,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::Error as McpError;

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
    let required_values: Vec<serde_json::Value> =
        required.iter().map(|s| serde_json::Value::String(s.to_string())).collect();
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
    vec![Content::text(serde_json::to_string(&json!({"error": {"code": code, "message": message}})).unwrap())]
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
    Tool::new("stock_pick", "Run multi-factor stock selection with LLM analysis.", Arc::new(make_schema(props, &[])))
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
                name: "sa".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some("Stock analysis engine. generate_guidance: daily market overview. stock_pick: multi-factor stock selection. report: per-symbol analysis. All tools accept market (a-share/hk/us) and lang (zh/en).".into()),
        }
    }

    async fn list_tools(
        &self,
        _req: PaginatedRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            next_cursor: None,
            tools: vec![tool_generate_guidance(), tool_stock_pick(), tool_generate_report()],
        })
    }

    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParam,
        _ctx: RequestContext<RoleServer>,
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
                        let mut out = json!(report);
                        let i18n = I18n::new();
                        out = bin_helpers::resolve_output(out, &i18n, lang);
                        Ok(CallToolResult::success(success_content(&out, self.compact_json)))
                    }
                    Err(e) => Ok(CallToolResult::success(error_content("guidance_failed", &e.to_string()))),
                }
            }
            "stock_pick" => {
                let market = args.get("market").and_then(|v| v.as_str()).unwrap_or("a-share");
                let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("zh");
                let date = args.get("date").and_then(|v| v.as_str()).map(String::from);
                let sector_type = args.get("sector_type").and_then(|v| v.as_str()).map(|s| s.to_string());
                let req = StockPickRequest {
                    market: market.to_string(),
                    analysis_date: date,
                    language: Some(lang.to_string()),
                    strategy: None,
                    candidate_symbols: None,
                    sector_type,
                    candidate_limit: None,
                    pick_count: None,
                    target_output_mode: None,
                    search_depth: None,
                    history_retrieval: None,
                };
                match stock_pick::run(&self.market_data, &self.llm, &req, None).await {
                    Ok(response) => {
                        let mut out = json!(response);
                        let i18n = I18n::new();
                        out = bin_helpers::resolve_output(out, &i18n, lang);
                        Ok(CallToolResult::success(success_content(&out, self.compact_json)))
                    }
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
                let mut result = json!({"symbol": symbol, "market": market});
                match generator.generate(&guidance_req).await {
                    Ok(report) => result["guidance"] = json!(report),
                    Err(e) => result["guidance_error"] = json!(e.to_string()),
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
                match stock_pick::run(&self.market_data, &self.llm, &pick_req, None).await {
                    Ok(response) => result["analysis"] = json!(response),
                    Err(e) => result["analysis_error"] = json!(e.to_string()),
                }
                if let Some(sections) = args.get("sections").and_then(|v| v.as_array()) {
                    result["requested_sections"] = json!(sections);
                }
                let i18n = I18n::new();
                let resolved = bin_helpers::resolve_output(result, &i18n, lang);
                Ok(CallToolResult::success(success_content(&resolved, self.compact_json)))
            }
            _ => Err(McpError::invalid_params(format!("unknown tool: {name}"), None)),
        }
    }
}

// ── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let compact_json = cli.json;

    match cli.command {
        // ── CLI commands ────────────────────────────────────────────────────
        Commands::Guidance { market, lang } => {
            let market_data = bin_helpers::build_market_data_client()
                .await
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));
            let memory = bin_helpers::build_memory();

            let generator = DailyGuidanceGenerator::new(market_data, memory);
            let generator = match bin_helpers::build_llm_client() {
                Ok(llm) => generator.with_llm(llm),
                Err(e) => {
                    tracing::warn!("LLM not available for sentiment enrichment: {e}");
                    generator
                }
            };
            let request = DailyGuidanceRequest {
                market: Some(market.as_str().to_string()),
                tickers: None,
                refresh: None,
                lang: lang.as_ref().map(|l| l.as_str().to_string()),
            };
            match generator.generate(&request).await {
                Ok(report) => {
                    let mut out = json!(report);
                    if let Some(l) = lang {
                        let i18n = I18n::new();
                        out = bin_helpers::resolve_output(out, &i18n, l.as_str());
                    }
                    print_json(&out, compact_json);
                }
                Err(e) => error_exit("guidance_failed", &e.to_string()),
            }
        }

        Commands::StockPick { market, date, candidate_symbols, sector_type, lang } => {
            let market_data = bin_helpers::build_market_data_client()
                .await
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));
            let llm = bin_helpers::build_llm_client()
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));

            let llm_lang = lang.map(|l| l.as_llm_lang().to_string());
            let request = StockPickRequest {
                market: market.as_str().to_string(),
                analysis_date: date,
                language: llm_lang.or_else(|| Some("zh-CN".to_string())),
                strategy: None,
                candidate_symbols,
                sector_type,
                candidate_limit: None,
                pick_count: None,
                target_output_mode: None,
                search_depth: None,
                history_retrieval: None,
            };
            match stock_pick::run(&market_data, &llm, &request, None).await {
                Ok(response) => {
                    let mut out = json!(response);
                    if let Some(l) = lang {
                        let i18n = I18n::new();
                        out = bin_helpers::resolve_output(out, &i18n, l.as_str());
                    }
                    print_json(&out, compact_json);
                }
                Err(e) => error_exit("stock_pick_failed", &e.to_string()),
            }
        }

        Commands::Report { symbol, market, sections, lang } => {
            let market_data = bin_helpers::build_market_data_client()
                .await
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));
            let llm = bin_helpers::build_llm_client()
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));
            let memory = bin_helpers::build_memory();

            let generator = DailyGuidanceGenerator::new(market_data.clone(), memory)
                .with_llm(llm.clone());
            let guidance_req = DailyGuidanceRequest {
                market: Some(market.as_str().to_string()),
                tickers: Some(vec![symbol.clone()]),
                refresh: None,
                lang: lang.as_ref().map(|l| l.as_str().to_string()),
            };
            let mut result = json!({"symbol": symbol, "market": market.as_str()});
            match generator.generate(&guidance_req).await {
                Ok(report) => result["guidance"] = json!(report),
                Err(e) => result["guidance_error"] = json!(e.to_string()),
            }
            let llm_lang = lang.map(|l| l.as_llm_lang().to_string());
            let pick_req = StockPickRequest {
                market: market.as_str().to_string(),
                candidate_symbols: Some(vec![symbol.clone()]),
                pick_count: Some(1),
                language: llm_lang.or_else(|| Some("zh-CN".to_string())),
                strategy: None,
                sector_type: None,
                candidate_limit: None,
                analysis_date: None,
                target_output_mode: None,
                search_depth: None,
                history_retrieval: None,
            };
            match stock_pick::run(&market_data, &llm, &pick_req, None).await {
                Ok(response) => result["analysis"] = json!(response),
                Err(e) => result["analysis_error"] = json!(e.to_string()),
            }
            if let Some(s) = sections {
                result["requested_sections"] = json!(s);
            }
            if let Some(l) = lang {
                let i18n = I18n::new();
                result = bin_helpers::resolve_output(result, &i18n, l.as_str());
            }
            print_json(&result, compact_json);
        }

        // ── MCP server ─────────────────────────────────────────────────────
        Commands::Mcp { transport, port, config } => {
            let mcp_key = load_mcp_key(config.as_deref());

            let market_data = bin_helpers::build_market_data_client()
                .await
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));
            let llm = bin_helpers::build_llm_client()
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));

            let server = StockAnalyzerServer { market_data, llm, compact_json };

            match transport.as_str() {
                "stdio" => {
                    use rmcp::ServiceExt;
                    let transport = rmcp::transport::stdio();
                    let running = server.serve(transport).await.expect("mcp serve failed");
                    running.waiting().await.expect("mcp waiting failed");
                }
                "http" => {
                    use rmcp::transport::SseServer;

                    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
                    let sse = SseServer::serve(addr).await.expect("bind failed");

                    // If mcp_key is set, log a warning about auth
                    if mcp_key.is_some() {
                        tracing::info!("MCP HTTP auth enabled (X-MCP-KEY required)");
                    } else {
                        tracing::warn!("MCP HTTP auth disabled — set mcp_key in config to enable X-MCP-KEY");
                    }

                    let ct = sse.with_service(move || server.clone());
                    eprintln!("MCP HTTP+SSE listening on {addr} (POST /message, GET /sse)");
                    ct.cancelled().await;
                }
                other => {
                    eprintln!("unknown transport: {other} (expected stdio or http)");
                    std::process::exit(1);
                }
            }
        }
    }
}

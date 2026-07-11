use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sa", about = "Stock analysis engine — CLI & MCP server")]
struct Cli {
    /// Output compact JSON (single line)
    #[arg(long, global = true)]
    json: bool,

    /// Language for i18n resolution (zh or en)
    #[arg(long, default_value = "zh", global = true)]
    lang: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show version info
    Version,
    /// Start MCP server
    #[cfg(feature = "mcp")]
    Mcp {
        /// Transport type (stdio or http)
        #[arg(long, default_value = "stdio")]
        transport: String,
        /// Port for HTTP transport
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Version => {
            serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "name": env!("CARGO_PKG_NAME"),
            })
        }
        #[cfg(feature = "mcp")]
        Commands::Mcp { transport, port } => run_mcp(&transport, port).await?,
    };

    if cli.json {
        let s = serde_json::to_string(&result)?;
        println!("{s}");
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}

#[cfg(feature = "mcp")]
async fn run_mcp(transport: &str, port: u16) -> Result<serde_json::Value> {
    match transport {
        "stdio" => {
            tracing::info!("Starting MCP server on stdio");
            // MCP server implementation would go here
            // For now, just return a placeholder
            Ok(serde_json::json!({"status": "mcp_stdio_not_implemented"}))
        }
        "http" => {
            tracing::info!("Starting MCP server on port {port}");
            // HTTP MCP server implementation would go here
            Ok(serde_json::json!({"status": "mcp_http_not_implemented", "port": port}))
        }
        _ => anyhow::bail!("Unsupported transport: {transport}. Use 'stdio' or 'http'."),
    }
}

//! sa-engine CLI — stock analysis engine command-line interface.

use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

use sa_engine::bin_helpers;
use sa_engine::engine::guidance::{DailyGuidanceGenerator, DailyGuidanceRequest};
use sa_engine::engine::stock_pick;
use sa_engine::i18n::I18n;
use sa_engine::models::StockPickRequest;
use sa_engine::types::MarketKind;

#[derive(Parser, Debug)]
#[command(name = "sa-engine", version, about = "Stock analysis engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate daily market guidance.
    Guidance {
        #[arg(long, value_enum, default_value_t = MarketArg::AShare)]
        market: MarketArg,
        #[arg(long, value_enum)]
        lang: Option<LangArg>,
    },

    /// Run stock selection.
    StockPick {
        #[arg(long, value_enum, default_value_t = MarketArg::AShare)]
        market: MarketArg,
        #[arg(long)]
        date: Option<String>,
        #[arg(long, value_enum)]
        lang: Option<LangArg>,
    },

    /// Generate analysis report.
    Report {
        #[arg(long)]
        symbol: String,
        #[arg(long, value_enum, default_value_t = MarketArg::AShare)]
        market: MarketArg,
        #[arg(long, value_delimiter = ',')]
        sections: Option<Vec<String>>,
        #[arg(long, value_enum)]
        lang: Option<LangArg>,
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
    #[allow(dead_code)]
    fn to_market_kind(self) -> MarketKind {
        match self {
            MarketArg::AShare => MarketKind::AShare,
            MarketArg::Hk => MarketKind::HongKong,
            MarketArg::Us => MarketKind::UsEquity,
        }
    }

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
        match self {
            LangArg::Zh => "zh",
            LangArg::En => "en",
        }
    }
}

fn error_exit(code: &str, message: &str) -> ! {
    eprintln!(
        "{}",
        serde_json::to_string(&json!({"error": {"code": code, "message": message}})).unwrap()
    );
    std::process::exit(1);
}

fn resolve_output(value: serde_json::Value, i18n: &I18n, lang: &str) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut resolved = serde_json::Map::new();
            for (k, v) in map {
                if k == "i18n_key" {
                    if let Some(key) = v.as_str() {
                        if let Some(text) = i18n.resolve(key, lang) {
                            resolved.insert("text".to_string(), json!(text));
                            resolved.insert("key".to_string(), json!(key));
                        }
                    }
                } else {
                    resolved.insert(k, resolve_output(v, i18n, lang));
                }
            }
            serde_json::Value::Object(resolved)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(|v| resolve_output(v, i18n, lang)).collect())
        }
        other => other,
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Guidance { market, lang } => {
            let market_data = bin_helpers::build_market_data_client()
                .await
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));
            let memory = bin_helpers::build_memory();
            let http = reqwest::Client::new();

            let generator = DailyGuidanceGenerator::new(market_data, memory, http);
            let request = DailyGuidanceRequest {
                market: Some(market.as_str().to_string()),
                tickers: None,
                refresh: None,
            };

            match generator.generate(&request).await {
                Ok(report) => {
                    let mut out = json!(report);
                    if let Some(l) = lang {
                        let i18n = I18n::new();
                        out = resolve_output(out, &i18n, l.as_str());
                    }
                    println!("{}", serde_json::to_string_pretty(&out).unwrap());
                }
                Err(e) => error_exit("guidance_failed", &e.to_string()),
            }
        }

        Commands::StockPick { market, date, lang } => {
            let market_data = bin_helpers::build_market_data_client()
                .await
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));
            let llm = bin_helpers::build_llm_client()
                .unwrap_or_else(|e| error_exit("init_failed", &e.to_string()));

            let request = StockPickRequest {
                market: market.as_str().to_string(),
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

            match stock_pick::run(&market_data, &llm, &request).await {
                Ok(response) => {
                    let mut out = json!(response);
                    if let Some(l) = lang {
                        let i18n = I18n::new();
                        out = resolve_output(out, &i18n, l.as_str());
                    }
                    println!("{}", serde_json::to_string_pretty(&out).unwrap());
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
            let http = reqwest::Client::new();

            // Generate guidance as context, then run stock pick for the specific symbol
            let generator = DailyGuidanceGenerator::new(market_data.clone(), memory, http);
            let guidance_req = DailyGuidanceRequest {
                market: Some(market.as_str().to_string()),
                tickers: Some(vec![symbol.clone()]),
                refresh: None,
            };

            let mut result = json!({
                "symbol": symbol,
                "market": market.as_str(),
            });

            // Generate guidance context
            match generator.generate(&guidance_req).await {
                Ok(report) => {
                    result["guidance"] = json!(report);
                }
                Err(e) => {
                    result["guidance_error"] = json!(e.to_string());
                }
            }

            // Run stock pick for this symbol
            let pick_req = StockPickRequest {
                market: market.as_str().to_string(),
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

            match stock_pick::run(&market_data, &llm, &pick_req).await {
                Ok(response) => {
                    result["analysis"] = json!(response);
                }
                Err(e) => {
                    result["analysis_error"] = json!(e.to_string());
                }
            }

            if let Some(s) = sections {
                result["requested_sections"] = json!(s);
            }

            if let Some(l) = lang {
                let i18n = I18n::new();
                result = resolve_output(result, &i18n, l.as_str());
            }

            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
    }
}

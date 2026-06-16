//! sa-engine CLI — stock analysis engine command-line interface.

use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

use sa_engine::bin_helpers;
use sa_engine::engine::guidance::{DailyGuidanceGenerator, DailyGuidanceRequest};
use sa_engine::engine::stock_pick;
use sa_engine::i18n::I18n;
use sa_engine::models::StockPickRequest;

#[derive(Parser, Debug)]
#[command(name = "sa-engine", version, about = "Stock analysis engine CLI")]
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
        #[arg(long, value_enum, default_value_t = MarketArg::AShare, help = "Target market")]
        market: MarketArg,
        #[arg(long, value_enum, help = "Output language")]
        lang: Option<LangArg>,
    },

    /// Run multi-factor stock selection with LLM analysis.
    StockPick {
        #[arg(long, value_enum, default_value_t = MarketArg::AShare, help = "Target market")]
        market: MarketArg,
        #[arg(long, help = "Analysis date (YYYY-MM-DD, default: today)")]
        date: Option<String>,
        #[arg(long, value_delimiter = ',', help = "Explicit symbols to evaluate")]
        candidate_symbols: Option<Vec<String>>,
        #[arg(long, value_enum, help = "Output language")]
        lang: Option<LangArg>,
    },

    /// Generate per-symbol analysis report.
    Report {
        #[arg(long, help = "Stock symbol (e.g. 600519.SH, 00700.HK, AAPL)")]
        symbol: String,
        #[arg(long, value_enum, default_value_t = MarketArg::AShare, help = "Target market")]
        market: MarketArg,
        #[arg(long, value_delimiter = ',', help = "Report sections to include")]
        sections: Option<Vec<String>>,
        #[arg(long, value_enum, help = "Output language")]
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

    /// Language string for the LLM request (e.g. "zh-CN", "en").
    fn as_llm_lang(self) -> &'static str {
        match self {
            LangArg::Zh => "zh-CN",
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

fn print_json(value: &serde_json::Value, compact: bool) {
    if compact {
        println!("{}", serde_json::to_string(value).unwrap());
    } else {
        println!("{}", serde_json::to_string_pretty(value).unwrap());
    }
}


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let compact_json = cli.json;

    match cli.command {
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

        Commands::StockPick { market, date, candidate_symbols, lang } => {
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

            // Generate guidance as context, then run stock pick for the specific symbol
            let generator = DailyGuidanceGenerator::new(market_data.clone(), memory)
                .with_llm(llm.clone());
            let guidance_req = DailyGuidanceRequest {
                market: Some(market.as_str().to_string()),
                tickers: Some(vec![symbol.clone()]),
                refresh: None,
                lang: lang.as_ref().map(|l| l.as_str().to_string()),
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
                result = bin_helpers::resolve_output(result, &i18n, l.as_str());
            }

            print_json(&result, compact_json);
        }
    }
}

//! sa-engine CLI — stock analysis engine command-line interface.

use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

#[allow(unused_imports)]
use sa_engine::types::MarketKind;

/// Stock analysis engine CLI.
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
        /// Target market.
        #[arg(long, value_enum, default_value_t = MarketArg::AShare)]
        market: MarketArg,

        /// Language for i18n keys.
        #[arg(long, value_enum)]
        lang: Option<LangArg>,
    },

    /// Run stock selection.
    StockPick {
        /// Target market.
        #[arg(long, value_enum, default_value_t = MarketArg::AShare)]
        market: MarketArg,

        /// Date in YYYY-MM-DD format (defaults to today).
        #[arg(long)]
        date: Option<String>,

        /// Language for i18n keys.
        #[arg(long, value_enum)]
        lang: Option<LangArg>,
    },

    /// Generate analysis report.
    Report {
        /// Ticker symbol (required).
        #[arg(long)]
        symbol: String,

        /// Target market.
        #[arg(long, value_enum, default_value_t = MarketArg::AShare)]
        market: MarketArg,

        /// Comma-separated section IDs to include.
        #[arg(long, value_delimiter = ',')]
        sections: Option<Vec<String>>,

        /// Language for i18n keys.
        #[arg(long, value_enum)]
        lang: Option<LangArg>,
    },
}

/// CLI market argument, maps to [`MarketKind`].
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

/// CLI language argument.
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Guidance { market, lang } => {
            let mut out = json!({
                "status": "not_implemented",
                "command": "guidance",
                "market": market.as_str(),
            });
            if let Some(l) = lang {
                out["lang"] = json!(l.as_str());
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        Commands::StockPick { market, date, lang } => {
            let mut out = json!({
                "status": "not_implemented",
                "command": "stock-pick",
                "market": market.as_str(),
            });
            if let Some(d) = date {
                out["date"] = json!(d);
            }
            if let Some(l) = lang {
                out["lang"] = json!(l.as_str());
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        Commands::Report {
            symbol,
            market,
            sections,
            lang,
        } => {
            let mut out = json!({
                "status": "not_implemented",
                "command": "report",
                "market": market.as_str(),
                "symbol": symbol,
            });
            if let Some(s) = sections {
                out["sections"] = json!(s);
            }
            if let Some(l) = lang {
                out["lang"] = json!(l.as_str());
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
    }
}

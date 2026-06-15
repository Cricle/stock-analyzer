pub(crate) mod indicators;
mod market_data;
mod news;
mod summarize;

use chrono::Utc;
use serde_json::{Value, json};

use crate::data::MarketDataClient;
use crate::models::{AnalysisScenarioData, PendingToolCall, ToolObservation};
use crate::types::{CandlePoint, NewsItem};

#[derive(Clone)]
pub struct TradingToolbox {
    market_data: MarketDataClient,
}

struct ToolExecutionResult {
    output: String,
    meta: Value,
}

impl TradingToolbox {
    const TECHNICAL_HISTORY_MIN_BARS: usize = 320;

    pub fn new(market_data: MarketDataClient) -> Self {
        Self { market_data }
    }

    fn summarize_success_output(tool_name: &str, output: &str, meta: &Value) -> String {
        match tool_name {
            "get_stock_data" => Self::summarize_stock_data_output(output),
            "get_indicators" => Self::summarize_indicator_output(output, meta),
            "get_fundamentals" | "get_balance_sheet" | "get_cashflow" | "get_income_statement" => {
                Self::summarize_json_object_output(output)
            }
            _ => output.to_string(),
        }
    }

    #[tracing::instrument(skip_all, fields(tool_name = %pending.tool_name, symbol = %symbol))]
    pub async fn execute(
        &self,
        symbol: &str,
        market_type: &str,
        scenario_data: Option<&AnalysisScenarioData>,
        pending: &PendingToolCall,
    ) -> ToolObservation {
        let result = self
            .execute_inner(
                symbol,
                market_type,
                scenario_data,
                &pending.tool_name,
                &pending.arguments,
            )
            .await;

        match result {
            Ok(result) => ToolObservation {
                tool_name: pending.tool_name.clone(),
                arguments: pending.arguments.clone(),
                output: Self::summarize_success_output(
                    &pending.tool_name,
                    &result.output,
                    &result.meta,
                ),
                meta: result.meta,
                success: true,
                created_at: Utc::now().to_rfc3339(),
            },
            Err(error) => ToolObservation {
                tool_name: pending.tool_name.clone(),
                arguments: pending.arguments.clone(),
                output: format!("tool execution failed: {error:#}"),
                meta: json!({
                    "kind": "tool_error",
                    "message": error.to_string(),
                }),
                success: false,
                created_at: Utc::now().to_rfc3339(),
            },
        }
    }

    async fn execute_inner(
        &self,
        symbol: &str,
        market_type: &str,
        scenario_data: Option<&AnalysisScenarioData>,
        tool_name: &str,
        arguments: &Value,
    ) -> anyhow::Result<ToolExecutionResult> {
        match tool_name {
            "get_stock_data" => {
                self.get_stock_data(symbol, market_type, scenario_data, arguments)
                    .await
            }
            "get_indicators" => self.get_indicators(symbol, scenario_data, arguments).await,
            "get_fundamentals" => self.get_fundamentals(symbol, scenario_data).await,
            "get_balance_sheet" => self.get_balance_sheet(symbol, scenario_data).await,
            "get_cashflow" => self.get_cashflow(symbol, scenario_data).await,
            "get_income_statement" => self.get_income_statement(symbol, scenario_data).await,
            "get_news" => self.get_news(symbol, scenario_data, arguments).await,
            "get_global_news" => self.get_global_news(symbol, market_type, arguments).await,
            "get_insider_transactions" => {
                self.get_insider_transactions(symbol, scenario_data, arguments)
                    .await
            }
            other => anyhow::bail!("unsupported tool: {other}"),
        }
    }

    fn filter_candles_by_date(
        &self,
        items: Vec<CandlePoint>,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> Vec<CandlePoint> {
        items
            .into_iter()
            .filter(|item| start_date.is_none_or(|value| item.trade_date.as_str() >= value))
            .filter(|item| end_date.is_none_or(|value| item.trade_date.as_str() <= value))
            .collect()
    }

    fn candle_fetch_limit(
        start_date: Option<&str>,
        end_date: Option<&str>,
        fallback: usize,
    ) -> usize {
        let Some(start_date) = start_date else {
            return fallback.max(Self::TECHNICAL_HISTORY_MIN_BARS);
        };
        let Ok(start) = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d") else {
            return fallback.max(Self::TECHNICAL_HISTORY_MIN_BARS);
        };
        let end = end_date
            .and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let today = chrono::Utc::now().date_naive();
        let effective_end = end.max(today);
        let span_days = (effective_end - start).num_days().unsigned_abs() as usize + 1;
        // Daily bars across historical windows need slack for weekends/holidays and indicators.
        fallback.max((span_days * 2).max(Self::TECHNICAL_HISTORY_MIN_BARS))
    }

    fn filter_news_by_date(
        &self,
        items: Vec<NewsItem>,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> Vec<NewsItem> {
        let mut filtered: Vec<_> = items
            .into_iter()
            .filter(|item| crate::data::news::within_date_window(&item.published_at, start_date, end_date))
            .collect();
        filtered.sort_by(|left, right| right.published_at.cmp(&left.published_at));
        filtered.dedup_by(|left, right| {
            left.published_at.get(0..10) == right.published_at.get(0..10)
                && left.title == right.title
                && left.source == right.source
        });
        filtered
    }

}

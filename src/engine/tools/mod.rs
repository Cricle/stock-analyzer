mod indicators;
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
                Self::summarize_json_object_output(output, 18)
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
        let meter = opentelemetry::global::meter("tradingagents");
        let tool_total = meter.u64_counter("tool_execution_total").build();
        let tool_duration = meter.f64_histogram("tool_execution_duration_ms").build();
        let tool_errors = meter.u64_counter("tool_execution_errors_total").build();

        let start = std::time::Instant::now();
        let result = self
            .execute_inner(
                symbol,
                market_type,
                scenario_data,
                &pending.tool_name,
                &pending.arguments,
            )
            .await;
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

        let base_attrs = [
            opentelemetry::KeyValue::new("tool.name", pending.tool_name.clone()),
            opentelemetry::KeyValue::new("tool.symbol", symbol.to_string()),
        ];

        match result {
            Ok(result) => {
                let mut attrs = base_attrs.to_vec();
                attrs.push(opentelemetry::KeyValue::new("tool.outcome", "success"));
                tool_total.add(1, &attrs);
                tool_duration.record(elapsed_ms, &attrs);
                ToolObservation {
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
                }
            }
            Err(error) => {
                let mut attrs = base_attrs.to_vec();
                attrs.push(opentelemetry::KeyValue::new("tool.outcome", "error"));
                tool_total.add(1, &attrs);
                tool_errors.add(1, &attrs);
                tool_duration.record(elapsed_ms, &attrs);
                ToolObservation {
                    tool_name: pending.tool_name.clone(),
                    arguments: pending.arguments.clone(),
                    output: format!("tool execution failed: {error:#}"),
                    meta: json!({
                        "kind": "tool_error",
                        "message": error.to_string(),
                    }),
                    success: false,
                    created_at: Utc::now().to_rfc3339(),
                }
            }
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
        let mut filtered = items
            .into_iter()
            .filter(|item| {
                if item.published_at.trim().is_empty() {
                    return true;
                }
                let normalized = crate::data::normalized_news_date(&item.published_at)
                    .unwrap_or_else(|| {
                        item.published_at
                            .get(0..10)
                            .unwrap_or(item.published_at.as_str())
                            .to_string()
                    });
                let date = normalized.as_str();
                start_date.is_none_or(|value| date >= value)
                    && end_date.is_none_or(|value| date <= value)
            })
            .collect::<Vec<_>>();
        filtered.sort_by(|left, right| right.published_at.cmp(&left.published_at));
        filtered.dedup_by(|left, right| {
            left.published_at.get(0..10) == right.published_at.get(0..10)
                && left.title == right.title
                && left.source == right.source
        });
        filtered
    }

    fn candle_json(item: &CandlePoint) -> Value {
        json!({
            "trade_date": item.trade_date,
            "open": item.open,
            "close": item.close,
            "high": item.high,
            "low": item.low,
            "volume": item.volume,
            "amount": item.amount,
            "amplitude_pct": item.amplitude_pct,
            "change_pct": item.change_pct,
            "change_amount": item.change_amount,
            "turnover_pct": item.turnover_pct
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn make_candle(date: &str) -> CandlePoint {
        CandlePoint {
            trade_date: date.to_string(),
            open: Decimal::new(100, 0),
            close: Decimal::new(105, 0),
            high: Decimal::new(110, 0),
            low: Decimal::new(95, 0),
            volume: 1000,
            amount: Decimal::new(100000, 0),
            amplitude_pct: 15.0,
            change_pct: 5.0,
            change_amount: Decimal::new(5, 0),
            turnover_pct: 1.0,
        }
    }

    fn make_news(date: &str, title: &str) -> NewsItem {
        NewsItem {
            published_at: date.to_string(),
            title: title.to_string(),
            summary: "summary".to_string(),
            source: "test".to_string(),
            url: None,
        }
    }

    fn test_toolbox() -> TradingToolbox {
        // Create a minimal toolbox for testing filter methods
        // These tests only exercise pure filtering logic, not HTTP calls
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = rt.block_on(MarketDataClient::new());
        TradingToolbox::new(client)
    }

    #[test]
    fn test_filter_candles_by_date_no_bounds() {
        let toolbox = test_toolbox();
        let items = vec![make_candle("2024-01-01"), make_candle("2024-06-15")];
        let result = toolbox.filter_candles_by_date(items, None, None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_candles_by_date_with_start() {
        let toolbox = test_toolbox();
        let items = vec![
            make_candle("2024-01-01"),
            make_candle("2024-06-15"),
            make_candle("2024-12-31"),
        ];
        let result = toolbox.filter_candles_by_date(items, Some("2024-06-01"), None);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].trade_date, "2024-06-15");
    }

    #[test]
    fn test_filter_candles_by_date_with_end() {
        let toolbox = test_toolbox();
        let items = vec![
            make_candle("2024-01-01"),
            make_candle("2024-06-15"),
            make_candle("2024-12-31"),
        ];
        let result = toolbox.filter_candles_by_date(items, None, Some("2024-06-30"));
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].trade_date, "2024-06-15");
    }

    #[test]
    fn test_filter_candles_by_date_with_both_bounds() {
        let toolbox = test_toolbox();
        let items = vec![
            make_candle("2024-01-01"),
            make_candle("2024-06-15"),
            make_candle("2024-12-31"),
        ];
        let result = toolbox.filter_candles_by_date(items, Some("2024-03-01"), Some("2024-09-01"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].trade_date, "2024-06-15");
    }

    #[test]
    fn test_filter_candles_by_date_empty() {
        let toolbox = test_toolbox();
        let result = toolbox.filter_candles_by_date(vec![], Some("2024-01-01"), Some("2024-12-31"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_candle_fetch_limit_no_dates() {
        let limit = TradingToolbox::candle_fetch_limit(None, None, 100);
        assert_eq!(limit, 320);
    }

    #[test]
    fn test_candle_fetch_limit_invalid_date() {
        let limit = TradingToolbox::candle_fetch_limit(Some("not-a-date"), None, 100);
        assert_eq!(limit, 320);
    }

    #[test]
    fn test_candle_fetch_limit_short_period() {
        let limit = TradingToolbox::candle_fetch_limit(Some("2024-06-01"), Some("2024-06-10"), 100);
        assert_eq!(limit, 320);
    }

    #[test]
    fn test_candle_fetch_limit_long_period() {
        let limit = TradingToolbox::candle_fetch_limit(Some("2020-01-01"), Some("2024-12-31"), 100);
        assert!(limit >= 3600);
    }

    #[test]
    fn test_candle_fetch_limit_large_fallback() {
        let limit = TradingToolbox::candle_fetch_limit(None, None, 5000);
        assert_eq!(limit, 5000);
    }

    #[test]
    fn test_filter_news_by_date_no_bounds() {
        let toolbox = test_toolbox();
        let items = vec![make_news("2024-01-01", "A"), make_news("2024-06-15", "B")];
        let result = toolbox.filter_news_by_date(items, None, None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_news_by_date_with_bounds() {
        let toolbox = test_toolbox();
        let items = vec![
            make_news("2024-01-01", "A"),
            make_news("2024-06-15", "B"),
            make_news("2024-12-31", "C"),
        ];
        let result = toolbox.filter_news_by_date(items, Some("2024-03-01"), Some("2024-09-01"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "B");
    }

    #[test]
    fn test_filter_news_by_date_empty_published_at() {
        let toolbox = test_toolbox();
        let items = vec![NewsItem {
            published_at: "".to_string(),
            title: "empty date".to_string(),
            summary: "s".to_string(),
            source: "test".to_string(),
            url: None,
        }];
        let result = toolbox.filter_news_by_date(items, Some("2024-01-01"), Some("2024-12-31"));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_news_by_date_dedup() {
        let toolbox = test_toolbox();
        let items = vec![
            make_news("2024-06-15", "Same Title"),
            make_news("2024-06-15", "Same Title"),
        ];
        let result = toolbox.filter_news_by_date(items, None, None);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_candle_json_keys() {
        let candle = make_candle("2024-01-15");
        let json = TradingToolbox::candle_json(&candle);
        assert_eq!(json["trade_date"], "2024-01-15");
        assert_eq!(json["volume"], 1000);
        assert!(json.get("open").is_some());
        assert!(json.get("close").is_some());
        assert!(json.get("high").is_some());
        assert!(json.get("low").is_some());
    }
}

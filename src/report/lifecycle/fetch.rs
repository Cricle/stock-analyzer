use chrono::Utc;

use crate::{AnalysisResult, PersistedTask, TaskManager};

// ---------------------------------------------------------------------------
// Market data fetch helpers
// ---------------------------------------------------------------------------

/// Core market data fetched for a fresh analysis run.
pub(super) struct CoreMarketData {
    pub(super) quote: Option<crate::data::QuoteSnapshot>,
    pub(super) fundamentals: Option<crate::data::FundamentalsSnapshot>,
    pub(super) news_items: Vec<crate::data::NewsItem>,
    pub(super) market_chart: crate::ReportMarketChart,
    pub(super) fetch_diagnosis: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// TaskManager impl — data fetching methods
// ---------------------------------------------------------------------------

impl TaskManager {
    pub(super) const MARKET_DATA_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
    pub(super) const MARKET_NEWS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

    /// Create a ParallelExecutor for data fetching.
    pub fn create_data_executor(&self) -> crate::data::pipeline::ParallelExecutor {
        let config = crate::data::pipeline::DataPipelineConfig::default();
        let cache = crate::data::cache::DataCacheLayer::new(
            config.cache_max_size,
            std::time::Duration::from_secs(config.cache_ttl_seconds),
        );
        let validator = crate::data::validator::DataValidator;
        crate::data::pipeline::ParallelExecutor::new(config, cache, validator)
    }

    /// Fetch core market data (quote, fundamentals, news, candles) for a fresh run.
    pub(super) async fn fetch_core_market_data(
        &self,
        task: &PersistedTask,
        news_start: Option<String>,
    ) -> CoreMarketData {
        let candle_limit = std::env::var("REPORT_KLINE_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(3000)
            .clamp(1, 5000);
        let (_quote_result, _fundamentals, _news_items, _candles_result) = tokio::join!(
            tokio::time::timeout(
                Self::MARKET_DATA_TIMEOUT,
                self.market_data.fetch_quote_with_rotation(&task.symbol)
            ),
            tokio::time::timeout(
                Self::MARKET_DATA_TIMEOUT,
                self.market_data.fetch_fundamentals(&task.symbol)
            ),
            tokio::time::timeout(
                Self::MARKET_NEWS_TIMEOUT,
                self.market_data.fetch_news(
                    &task.symbol,
                    15,
                    news_start.as_deref(),
                    Some(&task.analysis_date),
                )
            ),
            tokio::time::timeout(
                Self::MARKET_DATA_TIMEOUT,
                self.market_data
                    .fetch_candles_with_rotation(&task.symbol, "qfq", candle_limit)
            )
        );
        let (quote, quote_diagnosis) = match _quote_result {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!("quote fetch timed out for {}: {}", task.symbol, error);
                (
                    None,
                    crate::data::DataFetchDiagnosis::new("quote", &task.symbol),
                )
            }
        };
        if !quote_diagnosis.attempts.is_empty() {
            tracing::info!(
                task_id = %task.task_id,
                symbol = %task.symbol,
                diagnosis = %quote_diagnosis.summary(),
                "quote fetch diagnosis"
            );
        }
        let fundamentals = match _fundamentals {
            Ok(result) => result.ok(),
            Err(error) => {
                tracing::warn!(
                    "fundamentals fetch timed out for {}: {}",
                    task.symbol,
                    error
                );
                None
            }
        };
        let news_items = match _news_items {
            Ok(result) => result.unwrap_or_default(),
            Err(error) => {
                tracing::warn!("news fetch timed out for {}: {}", task.symbol, error);
                Vec::new()
            }
        };
        let (candles_data, candles_diagnosis) = match _candles_result {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!("candles fetch timed out for {}: {}", task.symbol, error);
                (
                    None,
                    crate::data::DataFetchDiagnosis::new("candles", &task.symbol),
                )
            }
        };
        if !candles_diagnosis.attempts.is_empty() {
            tracing::info!(
                task_id = %task.task_id,
                symbol = %task.symbol,
                diagnosis = %candles_diagnosis.summary(),
                "candles fetch diagnosis"
            );
        }
        let market_chart = match candles_data {
            Some(items) => {
                let provider_used = candles_diagnosis
                    .attempts
                    .last()
                    .filter(|a| a.success)
                    .map(|a| a.provider.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                crate::ReportMarketChart {
                    symbol: task.symbol.clone(),
                    market: task.market_type.clone(),
                    adjust: "qfq".to_string(),
                    source: self.market_data.candles_source(&task.symbol).to_string(),
                    provider_used,
                    start_date: items
                        .first()
                        .map(|item| item.trade_date.clone())
                        .unwrap_or_default(),
                    end_date: items
                        .last()
                        .map(|item| item.trade_date.clone())
                        .unwrap_or_default(),
                    candles: items
                        .into_iter()
                        .map(|item| crate::ReportCandle {
                            trade_date: item.trade_date,
                            open: item.open,
                            close: item.close,
                            high: item.high,
                            low: item.low,
                            volume: item.volume,
                            amount: item.amount,
                            amplitude_pct: item.amplitude_pct,
                            change_pct: item.change_pct,
                            change_amount: item.change_amount,
                            turnover_pct: item.turnover_pct,
                        })
                        .collect(),
                    indicators: Vec::new(),
                    overlays: Vec::new(),
                    trend_lines: Vec::new(),
                }
            }
            None => {
                tracing::warn!(
                    "candles fetch failed for {}: all providers exhausted",
                    task.symbol
                );
                crate::ReportMarketChart::default()
            }
        };
        let mut fetch_diagnosis = Vec::new();
        if !quote_diagnosis.attempts.is_empty() {
            fetch_diagnosis.push(serde_json::to_value(&quote_diagnosis).unwrap_or_default());
        }
        if !candles_diagnosis.attempts.is_empty() {
            fetch_diagnosis.push(serde_json::to_value(&candles_diagnosis).unwrap_or_default());
        }
        CoreMarketData {
            quote,
            fundamentals,
            news_items,
            market_chart,
            fetch_diagnosis,
        }
    }

    /// Fetch enrichment data and format summaries into the result.
    pub(super) async fn fetch_enrichment_and_store(
        &self,
        task: &PersistedTask,
        result: &mut AnalysisResult,
    ) {
        use super::format::{
            format_billboard_summary, format_earnings_forecast_summary, format_fund_flow_summary,
            format_hot_rank_summary, format_limit_pool_summary, format_margin_summary,
        };

        let (
            fund_flow_result,
            billboard_result,
            margin_result,
            hot_rank_result,
            earnings_result,
            zt_pool_result,
        ) = tokio::join!(
            tokio::time::timeout(
                Self::MARKET_DATA_TIMEOUT,
                self.market_data.fetch_fund_flow_individual(&task.symbol)
            ),
            tokio::time::timeout(
                Self::MARKET_DATA_TIMEOUT,
                self.market_data.fetch_lhb_stock_statistic(&task.symbol)
            ),
            tokio::time::timeout(
                Self::MARKET_DATA_TIMEOUT,
                self.market_data
                    .fetch_margin_ratio_pa(&task.symbol, &task.analysis_date)
            ),
            tokio::time::timeout(
                Self::MARKET_DATA_TIMEOUT,
                self.market_data.fetch_hot_follow_xq(&task.symbol)
            ),
            tokio::time::timeout(
                Self::MARKET_DATA_TIMEOUT,
                self.market_data
                    .fetch_earnings_forecast(&task.analysis_date)
            ),
            tokio::time::timeout(
                Self::MARKET_DATA_TIMEOUT,
                self.market_data.fetch_zt_pool(&task.analysis_date)
            ),
        );
        result.artifacts.scenario_data.fund_flow_summary =
            format_fund_flow_summary(&task.symbol, fund_flow_result);
        result.artifacts.scenario_data.billboard_summary =
            format_billboard_summary(&task.symbol, billboard_result);
        result.artifacts.scenario_data.margin_summary =
            format_margin_summary(&task.symbol, margin_result);
        result.artifacts.scenario_data.hot_rank_summary =
            format_hot_rank_summary(&task.symbol, hot_rank_result);
        result.artifacts.scenario_data.earnings_forecast_summary =
            format_earnings_forecast_summary(&task.symbol, earnings_result);
        result.artifacts.scenario_data.limit_pool_summary =
            format_limit_pool_summary(&task.symbol, zt_pool_result);
    }

    /// Hydrate scenario data from fetched market data into result artifacts.
    pub(super) fn hydrate_scenario_data(
        result: &mut AnalysisResult,
        market_chart: crate::ReportMarketChart,
        quote: &Option<crate::data::QuoteSnapshot>,
        fundamentals: &Option<crate::data::FundamentalsSnapshot>,
        news_items: &[crate::data::NewsItem],
        news_start: &Option<String>,
        task: &PersistedTask,
    ) {
        result.artifacts.market_chart = market_chart;
        result.artifacts.scenario_data.prefetched_at = Utc::now().to_rfc3339();
        result.artifacts.scenario_data.quote = quote.clone();
        result.artifacts.scenario_data.fundamentals = fundamentals.clone();
        result.artifacts.scenario_data.company_news = news_items.to_vec();
        result.artifacts.scenario_data.company_news_start_date = news_start.clone();
        result.artifacts.scenario_data.company_news_end_date = Some(task.analysis_date.clone());
        result.artifacts.scenario_data.candle_adjust = "qfq".to_string();
        result.artifacts.scenario_data.quote_status =
            if result.artifacts.scenario_data.quote.is_some() {
                "ok".to_string()
            } else {
                result.artifacts.scenario_data.add_issue(
                    "quote",
                    "quote_missing",
                    "warning",
                    format!("quote prefetch missing for {}", task.symbol),
                );
                "missing".to_string()
            };
        result.artifacts.scenario_data.fundamentals_status =
            if result.artifacts.scenario_data.fundamentals.is_some() {
                "ok".to_string()
            } else {
                result.artifacts.scenario_data.add_issue(
                    "fundamentals",
                    "fundamentals_missing",
                    "warning",
                    format!("fundamentals prefetch missing for {}", task.symbol),
                );
                "missing".to_string()
            };
        result.artifacts.scenario_data.company_news_status =
            if result.artifacts.scenario_data.company_news.is_empty() {
                result.artifacts.scenario_data.add_issue(
                    "news",
                    "company_news_sparse",
                    "warning",
                    format!(
                        "company news prefetch returned no items for {}",
                        task.symbol
                    ),
                );
                "sparse".to_string()
            } else {
                "ok".to_string()
            };
        result.artifacts.scenario_data.candles = result
            .artifacts
            .market_chart
            .candles
            .iter()
            .map(|item| crate::data::CandlePoint {
                trade_date: item.trade_date.clone(),
                open: item.open,
                close: item.close,
                high: item.high,
                low: item.low,
                volume: item.volume,
                amount: item.amount,
                amplitude_pct: item.amplitude_pct,
                change_pct: item.change_pct,
                change_amount: item.change_amount,
                turnover_pct: item.turnover_pct,
            })
            .collect();
        result.artifacts.scenario_data.candles_status =
            if result.artifacts.scenario_data.candles.is_empty() {
                result.artifacts.scenario_data.add_issue(
                    "candles",
                    "candles_missing",
                    "warning",
                    format!("candles prefetch returned no rows for {}", task.symbol),
                );
                "missing".to_string()
            } else {
                "ok".to_string()
            };
    }
}

pub(super) fn analysis_news_start(analysis_date: &str) -> Option<String> {
    chrono::NaiveDate::parse_from_str(analysis_date, "%Y-%m-%d")
        .ok()
        .map(|date| (date - chrono::Days::new(7)).format("%Y-%m-%d").to_string())
}

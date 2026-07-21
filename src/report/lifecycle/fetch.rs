use std::collections::BTreeMap;

use chrono::Utc;

use super::{DataDomain, DataProvenance, ReportQualityGate};
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
    pub(super) quality_gate: ReportQualityGate,
}

// ---------------------------------------------------------------------------
// TaskManager impl — data fetching methods
// ---------------------------------------------------------------------------

impl TaskManager {
    pub(super) const MARKET_DATA_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

    /// Create a ParallelExecutor for data fetching.
    pub fn create_data_executor(&self) -> crate::data::pipeline::ParallelExecutor {
        let config = crate::data::pipeline::DataPipelineConfig::default();
        let validator = crate::data::validator::DataValidator;
        crate::data::pipeline::ParallelExecutor::new(config, validator)
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
        let executor = self.create_data_executor();
        let symbol = &task.symbol;
        let (_quote_result, _fundamentals, _news_items, _candles_result) = tokio::join!(
            executor.fetch_with_retry("quote", || async {
                Ok(self.market_data.fetch_quote_with_rotation(symbol).await)
            }),
            executor.fetch_with_retry("fundamentals", || {
                self.market_data.fetch_fundamentals(symbol)
            }),
            executor.fetch_with_retry("news", || {
                self.market_data.fetch_news(
                    symbol,
                    15,
                    news_start.as_deref(),
                    Some(&task.analysis_date),
                )
            }),
            executor.fetch_with_retry("candles", || async {
                Ok(self
                    .market_data
                    .fetch_candles_with_rotation(symbol, "qfq", candle_limit)
                    .await)
            })
        );
        let (quote, quote_diagnosis) = match _quote_result {
            Some(result) => result,
            None => {
                tracing::warn!(
                    "quote fetch failed for {}: all retries exhausted",
                    task.symbol
                );
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
        let mut fundamentals = _fundamentals;
        let mut fundamentals_provenance = if fundamentals.is_some() {
            DataProvenance::successful("market_data", None, 1, false)
        } else {
            DataProvenance::failed("market_data", "primary fundamentals unavailable")
        };

        // Fallback for US equity when primary fundamentals are missing
        if fundamentals.is_none()
            && crate::env_config::fundamentals_fallback_enabled()
            && crate::AnalysisScenarioMarket::from_market_type(&task.market_type)
                == crate::AnalysisScenarioMarket::UsEquity
        {
            tracing::info!(
                symbol = %task.symbol,
                "primary fundamentals missing for US equity, trying fallback sources"
            );
            match self
                .market_data
                .fetch_us_fundamentals_yahoo(&task.symbol)
                .await
            {
                Ok(snapshot) => {
                    fundamentals = Some(snapshot);
                    let mut fallback = DataProvenance::successful("yahoo", None, 1, false);
                    fallback.attempts = fundamentals_provenance.attempts;
                    fallback.attempts.push(serde_json::json!({
                        "provider": "yahoo",
                        "success": true,
                    }));
                    fundamentals_provenance = fallback;
                }
                Err(error) => {
                    fundamentals_provenance.attempts.push(serde_json::json!({
                        "provider": "yahoo",
                        "success": false,
                        "error": error.to_string(),
                    }));
                }
            }
            if fundamentals.is_none() {
                match self
                    .market_data
                    .fetch_us_fundamentals_finnhub(&task.symbol)
                    .await
                {
                    Ok(snapshot) => {
                        fundamentals = Some(snapshot);
                        let mut fallback = DataProvenance::successful("finnhub", None, 1, false);
                        fallback.attempts = fundamentals_provenance.attempts;
                        fallback.attempts.push(serde_json::json!({
                            "provider": "finnhub",
                            "success": true,
                        }));
                        fundamentals_provenance = fallback;
                    }
                    Err(error) => {
                        fundamentals_provenance.attempts.push(serde_json::json!({
                            "provider": "finnhub",
                            "success": false,
                            "error": error.to_string(),
                        }));
                    }
                }
            }
        }

        let mut news_items = _news_items.unwrap_or_default();
        let mut news_provenance = if news_items.is_empty() {
            DataProvenance::failed("market_data", "primary company news unavailable")
        } else {
            DataProvenance::successful(
                "market_data",
                news_items.first().map(|item| item.published_at.clone()),
                news_items.len(),
                false,
            )
        };

        // Fallback for US equity when news is empty — try Finnhub company news
        if news_items.is_empty()
            && crate::AnalysisScenarioMarket::from_market_type(&task.market_type)
                == crate::AnalysisScenarioMarket::UsEquity
        {
            let news_start_date = news_start.as_deref().unwrap_or(&task.analysis_date);
            match self
                .market_data
                .fetch_us_news_finnhub(&task.symbol, news_start_date, &task.analysis_date)
                .await
            {
                Ok(items) if !items.is_empty() => {
                    tracing::info!(
                        symbol = %task.symbol,
                        count = items.len(),
                        "Finnhub company news fallback succeeded"
                    );
                    let mut fallback = DataProvenance::successful(
                        "finnhub",
                        items.first().map(|item| item.published_at.clone()),
                        items.len(),
                        false,
                    );
                    fallback.attempts = news_provenance.attempts;
                    fallback.attempts.push(serde_json::json!({
                        "provider": "finnhub",
                        "success": true,
                        "item_count": items.len(),
                    }));
                    news_provenance = fallback;
                    news_items = items;
                }
                Ok(_) => {
                    tracing::debug!(symbol = %task.symbol, "Finnhub company news returned no items");
                }
                Err(e) => {
                    tracing::debug!(symbol = %task.symbol, error = %e, "Finnhub company news fallback failed");
                    news_provenance.attempts.push(serde_json::json!({
                        "provider": "finnhub",
                        "success": false,
                        "error": e.to_string(),
                    }));
                }
            }
        }
        let (candles_data, candles_diagnosis) = match _candles_result {
            Some(result) => result,
            None => {
                tracing::warn!(
                    "candles fetch failed for {}: all retries exhausted",
                    task.symbol
                );
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
        let mut provenance = BTreeMap::new();
        provenance.insert(
            DataDomain::Quote,
            DataProvenance::from_diagnosis(
                &quote_diagnosis,
                quote.as_ref().map(|snapshot| snapshot.date.clone()),
                usize::from(quote.is_some()),
            ),
        );
        provenance.insert(
            DataDomain::Candles,
            DataProvenance::from_diagnosis(
                &candles_diagnosis,
                market_chart
                    .candles
                    .last()
                    .map(|candle| candle.trade_date.clone()),
                market_chart.candles.len(),
            ),
        );
        provenance.insert(DataDomain::Fundamentals, fundamentals_provenance);
        provenance.insert(DataDomain::CompanyNews, news_provenance);
        let quality_gate = ReportQualityGate::from_acquired_data(
            &quote,
            &market_chart.candles,
            &fundamentals,
            &news_items,
            provenance,
            Utc::now(),
        );
        fetch_diagnosis.push(serde_json::to_value(&quality_gate).unwrap_or_default());
        CoreMarketData {
            quote,
            fundamentals,
            news_items,
            market_chart,
            fetch_diagnosis,
            quality_gate,
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

use anyhow::Context;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use std::time::Instant;

use super::{build_user_context, build_user_context_prompt};
use crate::{TaskManager, TaskRunParams};
use sa_models::{AnalysisResult, PersistedTask, SingleAnalysisRequest, TaskEvent, TaskStatus};

fn trend_label(current: Option<f64>, reference: Option<f64>) -> &'static str {
    match current.zip(reference) {
        Some((price, level)) if price > level => "above",
        Some((price, level)) if price < level => "below",
        Some(_) => "at",
        None => "",
    }
}

fn rsi_label(value: f64) -> &'static str {
    if value > 70.0 {
        "overbought"
    } else if value < 30.0 {
        "oversold"
    } else {
        "neutral"
    }
}

fn adx_label(value: f64) -> &'static str {
    if value >= 25.0 {
        "strong_trend"
    } else if value <= 20.0 {
        "range_bound"
    } else {
        "moderate"
    }
}

// ---------------------------------------------------------------------------
// Market data fetch helpers
// ---------------------------------------------------------------------------

/// Core market data fetched for a fresh analysis run.
struct CoreMarketData {
    quote: Option<sa_data::QuoteSnapshot>,
    fundamentals: Option<sa_data::FundamentalsSnapshot>,
    news_items: Vec<sa_data::NewsItem>,
    market_chart: sa_models::ReportMarketChart,
    fetch_diagnosis: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Enrichment summary formatters
// ---------------------------------------------------------------------------

fn format_fund_flow_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<sa_data::FundFlowEntry>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let lines: Vec<String> = items
                .iter()
                .take(5)
                .map(|item| {
                    let price = item
                        .latest_price
                        .map(|v| format!("{:.2}", v))
                        .unwrap_or_else(|| "-".into());
                    let chg = item
                        .change_pct
                        .map(|v| format!("{:.2}", v))
                        .unwrap_or_else(|| "-".into());
                    let net = item
                        .net_flow
                        .map(|v| format!("{:.0}", v))
                        .unwrap_or_else(|| "-".into());
                    let to = item
                        .turnover_rate
                        .map(|v| format!("{:.2}", v))
                        .unwrap_or_else(|| "-".into());
                    format!(
                        "{}: price={} chg={}% net_flow={} turnover={}%",
                        item.code, price, chg, net, to
                    )
                })
                .collect();
            format!("Fund Flow:\n{}", lines.join("\n"))
        }
        Ok(Ok(_)) => "Fund Flow: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("fund flow fetch failed for {}: {}", symbol, e);
            "Fund Flow: fetch failed".to_string()
        }
        Err(_) => "Fund Flow: timeout".to_string(),
    }
}

fn format_billboard_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<sa_data::LhbStockStatistic>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let lines: Vec<String> = items.iter().take(3).map(|item| {
                format!("{} {} date={} chg={:.2}% net={:.0} billboard_times={} org_buy={} org_sell={}",
                    item.code, item.name, item.latest_trade_date, item.change_pct,
                    item.billboard_net_amount, item.billboard_times, item.org_buy_times, item.org_sell_times)
            }).collect();
            format!("Billboard:\n{}", lines.join("\n"))
        }
        Ok(Ok(_)) => "Billboard: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("billboard fetch failed for {}: {}", symbol, e);
            "Billboard: fetch failed".to_string()
        }
        Err(_) => "Billboard: timeout".to_string(),
    }
}

fn format_margin_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<sa_data::MarginRatioPa>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let lines: Vec<String> = items
                .iter()
                .take(3)
                .map(|item| {
                    format!(
                        "{} {}: fin_ratio={:.4} loan_ratio={:.4}",
                        item.code, item.name, item.fin_ratio, item.loan_ratio
                    )
                })
                .collect();
            format!("Margin Trading:\n{}", lines.join("\n"))
        }
        Ok(Ok(_)) => "Margin: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("margin fetch failed for {}: {}", symbol, e);
            "Margin: fetch failed".to_string()
        }
        Err(_) => "Margin: timeout".to_string(),
    }
}

fn format_hot_rank_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<sa_data::HotStockXq>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let lines: Vec<String> = items
                .iter()
                .take(3)
                .map(|item| {
                    format!(
                        "{} {} value={:.0} price={:.2}",
                        item.code, item.name, item.value, item.latest_price
                    )
                })
                .collect();
            format!("Hot Rank:\n{}", lines.join("\n"))
        }
        Ok(Ok(_)) => "Hot Rank: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("hot rank fetch failed for {}: {}", symbol, e);
            "Hot Rank: fetch failed".to_string()
        }
        Err(_) => "Hot Rank: timeout".to_string(),
    }
}

fn format_earnings_forecast_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<sa_data::EarningsForecast>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let matching: Vec<&sa_data::EarningsForecast> =
                items.iter().filter(|item| item.code == symbol).collect();
            if matching.is_empty() {
                format!(
                    "Earnings Forecast: {} entries on date, none for {}",
                    items.len(),
                    symbol
                )
            } else {
                let lines: Vec<String> = matching
                    .iter()
                    .take(3)
                    .map(|item| {
                        format!(
                            "{} {} type={} content={} range={}",
                            item.code,
                            item.name,
                            item.forecast_type,
                            item.forecast_content.as_deref().unwrap_or("-"),
                            item.change_range.as_deref().unwrap_or("-")
                        )
                    })
                    .collect();
                format!("Earnings Forecast:\n{}", lines.join("\n"))
            }
        }
        Ok(Ok(_)) => "Earnings Forecast: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("earnings forecast fetch failed for {}: {}", symbol, e);
            "Earnings Forecast: fetch failed".to_string()
        }
        Err(_) => "Earnings Forecast: timeout".to_string(),
    }
}

fn format_limit_pool_summary(
    symbol: &str,
    result: Result<anyhow::Result<Vec<sa_data::ZtPool>>, tokio::time::error::Elapsed>,
) -> String {
    match result {
        Ok(Ok(items)) if !items.is_empty() => {
            let matching: Vec<_> = items.iter().filter(|item| item.code == symbol).collect();
            if matching.is_empty() {
                format!(
                    "Limit-Up Pool: {} stocks hit limit up, {} not in pool",
                    items.len(),
                    symbol
                )
            } else {
                let lines: Vec<String> = matching
                    .iter()
                    .take(3)
                    .map(|item| {
                        format!(
                            "{} {} chg={:.2}% price={:.2} seals={} consec={} industry={}",
                            item.code,
                            item.name,
                            item.change_pct,
                            item.latest_price,
                            item.seal_amount,
                            item.consecutive_count,
                            item.industry
                        )
                    })
                    .collect();
                format!("Limit-Up Pool:\n{}", lines.join("\n"))
            }
        }
        Ok(Ok(_)) => "Limit-Up Pool: no data".to_string(),
        Ok(Err(e)) => {
            tracing::warn!("zt pool fetch failed for {}: {}", symbol, e);
            "Limit-Up Pool: fetch failed".to_string()
        }
        Err(_) => "Limit-Up Pool: timeout".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Technical indicator summary builder
// ---------------------------------------------------------------------------

fn build_technical_summary(chart: &sa_models::ReportMarketChart) -> String {
    if chart.candles.is_empty() {
        return "Technical Indicators: no candle data".to_string();
    }
    let current_price = chart.candles.last().map(|c| c.close);
    let candles = &chart.candles;
    let sma50 = sa_models::sma_report(candles, 50);
    let sma200 = sa_models::sma_report(candles, 200);
    let ema10 = sa_models::ema_report(candles, 10);
    let rsi14 = sa_models::rsi_report(candles, 14);
    let atr14 = sa_models::atr_report(candles, 14);
    let macd_vals = sa_models::macd_report(candles);
    let boll = sa_models::bollinger_report(candles, 20);
    let kdj = sa_models::kdj_report(candles, 9);
    let adx14 = sa_models::adx_report(candles, 14);
    let obv = sa_models::obv_report(candles);

    let mut lines = Vec::new();
    if let Some(price) = current_price {
        lines.push(format!("price={:.2}", price));
    }
    if let Some(v) = sma50 {
        lines.push(format!(
            "SMA50={:.2} {}",
            v,
            trend_label(current_price, Some(v))
        ));
    }
    if let Some(v) = sma200 {
        lines.push(format!(
            "SMA200={:.2} {}",
            v,
            trend_label(current_price, Some(v))
        ));
    }
    if let Some(v) = ema10 {
        lines.push(format!(
            "EMA10={:.2} {}",
            v,
            trend_label(current_price, Some(v))
        ));
    }
    if let Some(v) = rsi14 {
        lines.push(format!("RSI14={:.1} {}", v, rsi_label(v)));
    }
    if let Some(v) = atr14 {
        lines.push(format!("ATR14={:.2}", v));
    }
    if let Some((macd, signal, hist)) = macd_vals {
        lines.push(format!(
            "MACD={:.4} signal={:.4} hist={:.4}",
            macd, signal, hist
        ));
    }
    if let Some((mid, upper, lower)) = boll {
        lines.push(format!("BOLL={:.2}/{:.2}/{:.2}", upper, mid, lower));
    }
    if let Some((k, d, j)) = kdj {
        lines.push(format!("KDJ={:.1}/{:.1}/{:.1}", k, d, j));
    }
    if let Some(v) = adx14 {
        lines.push(format!("ADX14={:.1} {}", v, adx_label(v)));
    }
    if let Some((obv_val, delta)) = obv {
        lines.push(format!(
            "OBV={:.0} delta={:.0} {}",
            obv_val,
            delta,
            if delta > 0.0 {
                "accumulation"
            } else {
                "distribution"
            }
        ));
    }
    format!("Technical Indicators:\n{}", lines.join(", "))
}

// ---------------------------------------------------------------------------
// TaskManager impl — task lifecycle methods
// ---------------------------------------------------------------------------

impl TaskManager {
    const MARKET_DATA_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
    const MARKET_NEWS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

    pub async fn resume_task(&self, task_id: &str) -> anyhow::Result<bool> {
        let Some(task) = self.analysis_store.get_task(task_id).await? else {
            return Ok(false);
        };
        let checkpoint_step = self
            .checkpoint_store
            .checkpoint_step(&task.task_id, &task.symbol, &task.analysis_date)
            .await?;
        let can_resume_completed = checkpoint_step.is_some_and(|step| step < 100);
        if matches!(task.status, TaskStatus::Completed) && !can_resume_completed {
            return Ok(true);
        }
        if matches!(task.status, TaskStatus::Cancelled) {
            return Ok(false);
        }
        let params = self
            .task_run_params_from_request(&task, &task.request)
            .await;
        let this = self.clone();
        let task_id = task_id.to_string();
        let running_task_id = task_id.clone();
        let handle = tokio::spawn(async move {
            if let Err(error) = this.run_task(task_id.clone(), params).await {
                tracing::error!("resume task {} failed: {:?}", task_id, error);
                let _ = this
                    .publish_failure(
                        &task_id,
                        format!("Failed to resume analysis task: {error:#}"),
                    )
                    .await;
            }
        });
        self.running_tasks
            .write()
            .await
            .insert(running_task_id, handle.abort_handle());
        Ok(true)
    }

    pub async fn clear_task_checkpoint(&self, task_id: &str) -> anyhow::Result<bool> {
        let Some(task) = self.analysis_store.get_task(task_id).await? else {
            return Ok(false);
        };
        self.checkpoint_store
            .clear(&task.task_id, &task.symbol, &task.analysis_date)
            .await?;
        self.checkpoint_store
            .clear_graph_runtime(&task.task_id, &task.symbol, &task.analysis_date)
            .await?;
        Ok(true)
    }

    pub async fn task_run_params_from_request(
        &self,
        task: &PersistedTask,
        request: &SingleAnalysisRequest,
    ) -> TaskRunParams {
        let params = request.parameters.clone().unwrap_or_default();
        let user_context = build_user_context(&params);
        let memory_context = self
            .initial_memory_context(&task.symbol, &task.market_type, None, None, &[])
            .await;
        let scenario = sa_models::AnalysisScenarioContext::from_market_type(&task.market_type);
        let sector_context = self
            .fetch_sector_context_for_analysis(&task.market_type)
            .await;
        TaskRunParams {
            market_type: task.market_type.clone(),
            analysis_date: task.analysis_date.clone(),
            scenario,
            selected_analysts: params.selected_analysts.unwrap_or_default(),
            past_context: memory_context.context_text.clone(),
            memory_context: crate::task_manager::memory_snapshot_from_bundle(&memory_context),
            llm_base_url: params.llm_base_url,
            llm_api_key: params.llm_api_key,
            quick_analysis_model: params.quick_analysis_model,
            deep_analysis_model: params.deep_analysis_model,
            language: user_context.language.clone(),
            user_context_prompt: build_user_context_prompt(&user_context),
            user_context,
            sector_context,
        }
    }

    pub async fn mark_task_failed(
        &self,
        task_id: &str,
        error_message: String,
    ) -> anyhow::Result<()> {
        self.publish_failure(task_id, error_message).await
    }

    pub async fn cancel_task(&self, task_id: &str) -> anyhow::Result<bool> {
        let Some(mut task) = self.analysis_store.get_task(task_id).await? else {
            return Ok(false);
        };
        if matches!(
            task.status,
            TaskStatus::Completed | TaskStatus::Cancelled | TaskStatus::Failed
        ) {
            return Ok(false);
        }

        if let Some(handle) = self.running_tasks.write().await.remove(task_id) {
            handle.abort();
        }

        task.status = TaskStatus::Cancelled;
        task.current_step_name = "Task cancelled".to_string();
        task.current_step_description = "Cancelled by user".to_string();
        task.message = "Task cancelled".to_string();
        task.error_message = Some("cancelled_by_user".to_string());
        task.updated_at = Utc::now();
        self.update_task(
            task_id,
            TaskStatus::Cancelled,
            task.progress,
            &task.current_step_name,
            &task.current_step_description,
            &task.message,
            task.error_message.clone(),
        )
        .await?;

        let params = self
            .task_run_params_from_request(&task, &task.request)
            .await;
        let mut result = match self.analysis_store.get_result(task_id).await? {
            Some(result) => result,
            None => {
                crate::analysis::runtime::TradingAgentsGraph::prepare_result(self, &task, &params)
                    .await?
            }
        };
        result.agent_state.company_of_interest = result.symbol.clone();
        result.agent_state.trade_date = result.analysis_date.clone();
        result.agent_state.sender = "System".to_string();
        result.agent_state.past_context = params.past_context.clone();
        result.artifacts.user_context = params.user_context.clone();
        self.refresh_structured_report_snapshot(&mut result).await?;
        result.apply_calibrated_markdown();
        self.analysis_store.save_result(task_id, &result).await?;

        // Store analysis summary in Qdrant for cross-module retrieval
        if let Err(e) = store_analysis_in_qdrant(&result).await {
            tracing::warn!(symbol = %result.symbol, error = %e, "failed to store analysis in qdrant");
        }

        Ok(true)
    }

    pub async fn execute_existing_task(
        &self,
        task_id: String,
        params: TaskRunParams,
    ) -> anyhow::Result<()> {
        let this = self.clone();
        let running_task_id = task_id.clone();
        let handle = tokio::spawn(async move {
            if let Err(error) = this.run_task(task_id.clone(), params).await {
                tracing::error!("worker task {} failed: {:?}", task_id, error);
                let _ = this
                    .publish_failure(&task_id, format!("Analysis task failed: {error:#}"))
                    .await;
            }
        });
        self.running_tasks
            .write()
            .await
            .insert(running_task_id, handle.abort_handle());
        Ok(())
    }
}
impl TaskManager {
    /// Fetch core market data (quote, fundamentals, news, candles) for a fresh run.
    async fn fetch_core_market_data(
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
                    sa_data::diagnosis::DataFetchDiagnosis::new("quote", &task.symbol),
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
                    sa_data::diagnosis::DataFetchDiagnosis::new("candles", &task.symbol),
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
                sa_models::ReportMarketChart {
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
                        .map(|item| sa_models::ReportCandle {
                            trade_date: item.trade_date,
                            open: item.open.to_f64().unwrap_or_default(),
                            close: item.close.to_f64().unwrap_or_default(),
                            high: item.high.to_f64().unwrap_or_default(),
                            low: item.low.to_f64().unwrap_or_default(),
                            volume: item.volume,
                            amount: item.amount.to_f64().unwrap_or_default(),
                            amplitude_pct: item.amplitude_pct,
                            change_pct: item.change_pct,
                            change_amount: item.change_amount.to_f64().unwrap_or_default(),
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
                sa_models::ReportMarketChart::default()
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
    async fn fetch_enrichment_and_store(&self, task: &PersistedTask, result: &mut AnalysisResult) {
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
    fn hydrate_scenario_data(
        result: &mut AnalysisResult,
        market_chart: sa_models::ReportMarketChart,
        quote: &Option<sa_data::QuoteSnapshot>,
        fundamentals: &Option<sa_data::FundamentalsSnapshot>,
        news_items: &[sa_data::NewsItem],
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
            .map(|item| sa_data::CandlePoint {
                trade_date: item.trade_date.clone(),
                open: Decimal::from_f64(item.open).unwrap_or_default(),
                close: Decimal::from_f64(item.close).unwrap_or_default(),
                high: Decimal::from_f64(item.high).unwrap_or_default(),
                low: Decimal::from_f64(item.low).unwrap_or_default(),
                volume: item.volume,
                amount: Decimal::from_f64(item.amount).unwrap_or_default(),
                amplitude_pct: item.amplitude_pct,
                change_pct: item.change_pct,
                change_amount: Decimal::from_f64(item.change_amount).unwrap_or_default(),
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

    pub(super) async fn run_task(
        &self,
        task_id: String,
        params: TaskRunParams,
    ) -> anyhow::Result<()> {
        let task_started_at = Instant::now();
        self.update_task(
            &task_id,
            TaskStatus::Running,
            5,
            "Preparing analysis",
            "Initializing task and loading context",
            "Preparing analysis",
            None,
        )
        .await?;

        let task = self
            .analysis_store
            .get_task(&task_id)
            .await?
            .context("task missing after execution")?;
        crate::telemetry::mark_span_task(&task.task_id, &task.symbol, &task.market_type);
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            market_type = %task.market_type,
            analysis_date = %task.analysis_date,
            owner_username = %task.owner_username,
            "analysis task started"
        );
        if let Err(error) = self.resolve_pending_entries(&task.symbol, &params).await {
            tracing::warn!(
                "failed to resolve pending memory entries for {} before analysis: {:?}",
                task.symbol,
                error
            );
        }

        // Prepare result early so we can check for checkpoint resume data
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: prepare_result start (early)"
        );
        let mut result =
            crate::analysis::runtime::TradingAgentsGraph::prepare_result(self, &task, &params)
                .await?;
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            resumed_from_step = result.artifacts.resumed_from_step,
            has_cached_quote = result.artifacts.scenario_data.quote.is_some(),
            has_cached_news = !result.artifacts.scenario_data.company_news.is_empty(),
            "run_task: prepare_result done (early)"
        );

        let is_resume_with_data = result.artifacts.resumed_from_step > 0
            && result.artifacts.scenario_data.quote.is_some();

        let (quote, fundamentals, news_items, market_chart, news_start);
        let params = params;

        let _hydration_span = tracing::info_span!(
            "analysis.market_data_hydration",
            task_id = %task.task_id,
            symbol = %task.symbol,
            market_type = %task.market_type,
            is_resume = is_resume_with_data,
        );

        if is_resume_with_data {
            tracing::info!(
                task_id = %task.task_id,
                symbol = %task.symbol,
                "run_task: resuming from checkpoint, reusing cached market data"
            );
            quote = result.artifacts.scenario_data.quote.clone();
            fundamentals = result.artifacts.scenario_data.fundamentals.clone();
            news_items = result.artifacts.scenario_data.company_news.clone();
            news_start = result
                .artifacts
                .scenario_data
                .company_news_start_date
                .clone();
            market_chart = result.artifacts.market_chart.clone();
        } else {
            // Fresh run — fetch market data via helpers
            let news_start_val = analysis_news_start(&task.analysis_date);
            news_start = news_start_val.clone();

            let core_data = self.fetch_core_market_data(&task, news_start_val).await;
            self.fetch_enrichment_and_store(&task, &mut result).await;

            result.artifacts.scenario_data.fetch_diagnosis = core_data.fetch_diagnosis;
            result.artifacts.scenario_data.technical_summary =
                build_technical_summary(&core_data.market_chart);

            quote = core_data.quote;
            fundamentals = core_data.fundamentals;
            news_items = core_data.news_items;
            market_chart = core_data.market_chart;
        } // end else (fresh run)

        let refined_memory_context = self
            .initial_memory_context(
                &task.symbol,
                &task.market_type,
                quote.as_ref(),
                fundamentals.as_ref(),
                &news_items,
            )
            .await;
        let mut params = params;
        params.past_context = refined_memory_context.context_text.clone();
        params.memory_context =
            crate::task_manager::memory_snapshot_from_bundle(&refined_memory_context);

        result.artifacts.scenario_context = params.scenario.clone();
        if !is_resume_with_data {
            Self::hydrate_scenario_data(
                &mut result,
                market_chart,
                &quote,
                &fundamentals,
                &news_items,
                &news_start,
                &task,
            );
        }

        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            quote_present = result.artifacts.scenario_data.quote.is_some(),
            fundamentals_present = result.artifacts.scenario_data.fundamentals.is_some(),
            company_news_count = result.artifacts.scenario_data.company_news.len(),
            candles_count = result.artifacts.scenario_data.candles.len(),
            chart_candles_count = result.artifacts.market_chart.candles.len(),
            "run_task: scenario hydration done"
        );
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: refresh_structured_report_snapshot start"
        );
        let _ = self.refresh_structured_report_snapshot(&mut result).await;
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: refresh_structured_report_snapshot done"
        );
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: initial save_result start"
        );
        self.analysis_store.save_result(&task_id, &result).await?;
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: initial save_result done"
        );

        self.update_task(
            &task_id,
            TaskStatus::Running,
            10,
            "Analyst phase",
            "Running analyst agents",
            "Analyst phase running",
            None,
        )
        .await?;

        drop(_hydration_span);

        let _graph_span = tracing::info_span!(
            "analysis.graph_execution",
            task_id = %task.task_id,
            symbol = %task.symbol,
        );

        if let Err(error) = self
            .run_agent_graph(
                &mut result,
                &params,
                quote.as_ref(),
                fundamentals.as_ref(),
                &news_items,
            )
            .await
        {
            let _ = self
                .finalize_partial_result_on_failure(&mut result, &params)
                .await;
            let _ = self.analysis_store.save_result(&task_id, &result).await;
            return Err(error);
        }

        // Run consistency validator after all LLM stages complete, before saving.
        let consistency_issues =
            crate::analysis::diagnosis::ConsistencyValidator::validate_and_fix(&mut result);
        if !consistency_issues.is_empty() {
            tracing::info!(
                task_id = %task_id,
                issues_count = consistency_issues.len(),
                "consistency validator applied auto-fixes"
            );
            result.artifacts.diagnosis_summary = Some(sa_models::DiagnosisSummary::from_issues(
                &consistency_issues,
            ));
        }

        self.analysis_store
            .save_result(&task_id, &result)
            .await
            .with_context(|| format!("failed to save final result for task {task_id}"))?;

        drop(_graph_span);
        let _finalization_span = tracing::info_span!(
            "analysis.result_finalization",
            task_id = %task.task_id,
            symbol = %task.symbol,
        );

        self.save_checkpoint(
            &task_id,
            &task.symbol,
            &task.analysis_date,
            "complete",
            "complete",
            &result,
        )
        .await
        .with_context(|| format!("failed to save final checkpoint for task {task_id}"))?;
        let _ = self
            .memory_log
            .store_decision_async(
                &task.symbol,
                &task.analysis_date,
                &result.agent_state.final_trade_decision,
                result.report.recommendation.as_str(),
                result.report.trader_plan.action.as_str(),
                &result.market_type,
                result.report.direction_score,
                result.report.confidence_score,
                result.report.action_score,
                Some(&crate::memory::ResearchMemoryRecord {
                    setup_tags: sa_models::derive_setup_tags(
                        &result.report.confidence_breakdown,
                        &result.report.direction_breakdown,
                        &result.report.execution_readiness,
                        &result.report.research_plan,
                        &result.report.trader_plan,
                        &result.report.portfolio_decision,
                    ),
                    stock_name: result.stock_name.clone(),
                    summary: result.report.summary.key.clone(),
                    risk_assessment: result.report.risk_assessment.key.clone(),
                    rationale: result.report.rationale.key.clone(),
                    structured_risk: sa_models::StructuredRiskAssessment::from_text(
                        result.report.risk_assessment.as_str(),
                    ),
                    structured_reflection: result.report.reflection.clone(),
                    trigger_checklist: result
                        .report
                        .portfolio_decision
                        .trigger_checklist
                        .iter()
                        .chain(result.report.trader_plan.execution_trigger_checklist.iter())
                        .cloned()
                        .collect(),
                    blocking_gaps: result
                        .report
                        .portfolio_decision
                        .missing_evidence_ladder
                        .blocking_gaps
                        .iter()
                        .chain(result.report.trader_plan.blocking_gaps.iter())
                        .cloned()
                        .collect(),
                    execution_boundary_complete: result
                        .report
                        .execution_readiness
                        .execution_boundary_complete,
                    structured_snapshot: serde_json::json!({
                        "market_chart": result.report.market_chart,
                        "price_context": result.report.price_context,
                        "probability_view": result.report.probability_view,
                        "profit_risk": result.report.profit_risk,
                        "ic_navigator": result.report.ic_navigator,
                        "technical_indicators": result.report.technical_indicators,
                        "evidence_cards": result.report.evidence_cards,
                        "news_insights": result.report.news_insights,
                        "risk_controls": result.report.risk_controls,
                    }),
                }),
                &task.owner_username,
            )
            .await;
        self.checkpoint_store
            .clear(&task.task_id, &task.symbol, &task.analysis_date)
            .await
            .with_context(|| format!("failed to clear checkpoint for task {task_id}"))?;
        self.checkpoint_store
            .clear_graph_runtime(&task.task_id, &task.symbol, &task.analysis_date)
            .await
            .with_context(|| {
                format!("failed to clear graph runtime checkpoint for task {task_id}")
            })?;

        self.update_task(
            &task_id,
            TaskStatus::Completed,
            100,
            "Analysis completed",
            "Report and decision generated",
            "Analysis completed",
            None,
        )
        .await
        .with_context(|| format!("failed to publish completed status for task {task_id}"))?;
        crate::telemetry::record_analysis_task_duration(
            &self.telemetry,
            "completed",
            &task.market_type,
            task_started_at.elapsed().as_secs_f64() * 1000.0,
            None,
        );
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            market_type = %task.market_type,
            elapsed_ms = task_started_at.elapsed().as_secs_f64() * 1000.0,
            "analysis task completed"
        );
        let _ = self.cache_completed_analysis(&task, &result).await;
        self.running_tasks.write().await.remove(&task_id);
        Ok(())
    }
}
impl TaskManager {
    pub(super) async fn publish_failure(
        &self,
        task_id: &str,
        error_message: String,
    ) -> anyhow::Result<()> {
        let task = self.analysis_store.get_task(task_id).await?;
        let market_type = task
            .as_ref()
            .map(|task| task.market_type.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let reason = if error_message.to_ascii_lowercase().contains("timeout") {
            "timeout"
        } else if error_message.contains("worker") {
            "worker_failure"
        } else if error_message.contains("LLM")
            || error_message.to_ascii_lowercase().contains("model")
        {
            "llm_failure"
        } else if error_message.to_ascii_lowercase().contains("database") {
            "database_failure"
        } else {
            "internal_error"
        };
        self.update_task(
            task_id,
            TaskStatus::Failed,
            100,
            "Analysis failed",
            "An error occurred during execution",
            &error_message,
            Some(error_message.clone()),
        )
        .await?;
        crate::telemetry::record_analysis_task_duration(
            &self.telemetry,
            "failed",
            &market_type,
            0.0,
            Some(reason),
        );
        tracing::error!(
            task_id = %task_id,
            market_type = %market_type,
            analysis_date = task.as_ref().map(|task| task.analysis_date.as_str()).unwrap_or("unknown"),
            symbol = task.as_ref().map(|task| task.symbol.as_str()).unwrap_or("unknown"),
            owner_username = task.as_ref().map(|task| task.owner_username.as_str()).unwrap_or(""),
            error_reason = reason,
            error_message = %error_message,
            "analysis task failed"
        );
        self.running_tasks.write().await.remove(task_id);
        Ok(())
    }

    pub(crate) async fn update_task(
        &self,
        task_id: &str,
        status: TaskStatus,
        progress: i32,
        step_name: &str,
        step_description: &str,
        message: &str,
        error_message: Option<String>,
    ) -> anyhow::Result<()> {
        tracing::info!(task_id, progress, step_name, "update_task: load task start");
        let mut task = self
            .analysis_store
            .get_task(task_id)
            .await?
            .context("task not found")?;
        tracing::info!(task_id, progress, step_name, "update_task: load task done");
        task.status = status.clone();
        task.progress = progress;
        task.current_step_name = step_name.to_string();
        task.current_step_description = step_description.to_string();
        task.message = message.to_string();
        task.error_message = error_message.clone();
        task.updated_at = Utc::now();
        tracing::info!(
            task_id,
            progress,
            step_name,
            "update_task: store update start"
        );
        self.analysis_store.update_task(&task).await?;
        tracing::info!(
            task_id,
            progress,
            step_name,
            "update_task: store update done"
        );
        // Avoid deserializing the full result snapshot during hot progress updates.
        // This path only feeds websocket metadata and was causing stack overflows
        // under large in-flight analysis payloads.
        let result_stage = None;

        tracing::info!(
            task_id,
            progress,
            step_name,
            "update_task: broadcaster start"
        );
        let sender = self.broadcaster(task_id).await;
        tracing::info!(
            task_id,
            progress,
            step_name,
            "update_task: broadcaster done"
        );
        let event = TaskEvent {
            event_type: "progress_update".to_string(),
            task_id: task_id.to_string(),
            status,
            progress,
            message: message.to_string(),
            current_step_name: step_name.to_string(),
            current_step_description: step_description.to_string(),
            emitted_at: Utc::now().to_rfc3339(),
            result_stage,
            llm_token_usage: task.llm_token_usage,
        };
        let _ = sender.send(event.clone());
        // Publish to Redis pub/sub for cross-instance delivery
        self.publish_task_event(task_id, &event).await;
        tracing::info!(task_id, progress, step_name, "update_task: send done");
        Ok(())
    }

    pub(crate) async fn save_checkpoint(
        &self,
        task_id: &str,
        symbol: &str,
        analysis_date: &str,
        stage: &str,
        node: &str,
        result: &AnalysisResult,
    ) -> anyhow::Result<()> {
        self.checkpoint_store
            .save(&crate::checkpoint::TaskCheckpoint {
                task_id: task_id.to_string(),
                symbol: symbol.to_string(),
                analysis_date: analysis_date.to_string(),
                stage: stage.to_string(),
                node: node.to_string(),
                result: result.clone(),
                step: crate::TaskManager::stage_step(stage),
            })
            .await
    }
}

fn analysis_news_start(analysis_date: &str) -> Option<String> {
    chrono::NaiveDate::parse_from_str(analysis_date, "%Y-%m-%d")
        .ok()
        .map(|date| (date - chrono::Days::new(7)).format("%Y-%m-%d").to_string())
}

/// Store analysis result summary in the stock_pick Qdrant collection
/// so stock picks and guidance can retrieve it.
async fn store_analysis_in_qdrant(result: &sa_models::AnalysisResult) -> anyhow::Result<()> {
    let qdrant_url = std::env::var("QDRANT_URL")
        .or_else(|_| std::env::var("RAG_QDRANT_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:6333".to_string());
    let collection = "tradingagents_stock_pick";

    let report = &result.report;
    let rating = report.recommendation.clone();
    let confidence = report.confidence_score;
    let entry_price = report.decision_view.current_price.clone();
    let target_price = report.decision_view.target_reference.clone();
    let stop_loss = report.decision_view.invalidation_level.clone();
    let reader_summary = report.decision_view.reader_summary.clone();

    // Extract key news headlines
    let news_headlines: Vec<String> = report
        .news_insights
        .iter()
        .take(5)
        .map(|n| n.title.clone())
        .collect();

    // Extract key technical signals
    let technical_summary: Vec<String> = report
        .technical_indicators
        .categories
        .iter()
        .flat_map(|c| c.indicators.iter())
        .filter(|i| !i.signal_code.is_empty())
        .take(5)
        .map(|i| format!("{}: {:?}", i.key, i.value.unwrap_or(0.0)))
        .collect();

    let summary_text = format!(
        "{} {} market {} analysis {} conclusion {} rating {}",
        result.symbol,
        result.stock_name,
        result.market_type,
        result.analysis_date,
        reader_summary,
        rating
    );

    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    let point_id = {
        let digest = Sha256::digest(format!(
            "analysis:{}:{}:{}",
            result.symbol, result.analysis_date, result.task_id
        ));
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes).to_string()
    };

    let embedding = crate::guidance::semantic_embed(&summary_text);

    let client = crate::shared::shared_http_client();
    let resp = client
        .put(format!(
            "{}/collections/{}/points?wait=true",
            qdrant_url.trim().trim_end_matches('/'),
            collection
        ))
        .json(&serde_json::json!({
            "points": [{
                "id": point_id,
                "vector": embedding,
                "payload": {
                    "entry_kind": "analysis_result",
                    "symbol": result.symbol,
                    "stock_name": result.stock_name,
                    "market": result.market_type,
                    "analysis_date": result.analysis_date,
                    "task_id": result.task_id,
                    "rating": rating.clone(),
                    "confidence": confidence as i64,
                    "entry_price": entry_price,
                    "target_price": target_price,
                    "stop_loss": stop_loss,
                    "reader_summary": reader_summary,
                    "news_headlines": news_headlines,
                    "technical_summary": technical_summary,
                    "created_at": result.created_at
                }
            }]
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("qdrant analysis store failed: {body}");
    }

    tracing::info!(
        symbol = %result.symbol,
        analysis_date = %result.analysis_date,
        "analysis result stored in qdrant"
    );
    Ok(())
}

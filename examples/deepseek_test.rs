use std::io::Write;
use std::sync::Arc;

use sa::{InMemoryAnalysisStore, InMemoryCacheStore, InMemoryCheckpointStore};

async fn wait_for_task(
    manager: &sa::TaskManager,
    task_id: &str,
    timeout: std::time::Duration,
) -> sa::PersistedTask {
    let deadline = std::time::Instant::now() + timeout;
    let start = std::time::Instant::now();
    loop {
        let task = manager
            .analysis_store()
            .get_task(task_id)
            .await
            .unwrap()
            .expect("task should exist");
        match task.status {
            sa::TaskStatus::Completed | sa::TaskStatus::Failed => {
                println!(
                    "  [{:.0}s] {:?} {}",
                    start.elapsed().as_secs_f64(),
                    task.status,
                    task.error_message.as_deref().unwrap_or("")
                );
                let _ = std::io::stdout().flush();
                return task;
            }
            _ => {
                if start.elapsed().as_secs() % 15 == 0 {
                    print!(".");
                    let _ = std::io::stdout().flush();
                }
            }
        }
        if std::time::Instant::now() > deadline {
            println!("  ⏰ TIMEOUT after {:.0}s", start.elapsed().as_secs_f64());
            return task;
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

fn print_detailed_indicators(result: &sa::AnalysisResult) {
    let r = &result.report;

    println!("  ┌─ 核心评分 ──────────────────────────────────────────");
    println!("  │ 方向分 (direction_score): {}", r.direction_score);
    println!("  │   市场: {}  基本面: {}  舆情: {}  情绪: {}  风险调整: {}",
        r.direction_breakdown.market.score,
        r.direction_breakdown.fundamentals.score,
        r.direction_breakdown.news.score,
        r.direction_breakdown.sentiment.score,
        r.direction_breakdown.risk_adjustment.score,
    );
    println!("  │   隐含评级: {}", r.direction_breakdown.implied_rating.key);
    println!("  │ 行动分 (action_score): {}", r.action_score);
    println!("  │ 置信度 (confidence_score): {}", r.confidence_score);
    println!("  │   数据质量: {}  趋势确认: {}  基本面确认: {}  催化剂: {}",
        r.confidence_breakdown.data_quality.score,
        r.confidence_breakdown.trend_confirmation.score,
        r.confidence_breakdown.fundamental_confirmation.score,
        r.confidence_breakdown.catalyst_quality.score,
    );
    println!("  │   历史可迁移: {}  跨代理一致: {}  风险清晰: {}",
        r.confidence_breakdown.historical_transferability.score,
        r.confidence_breakdown.cross_agent_consistency.score,
        r.confidence_breakdown.risk_clarity.score,
    );
    println!("  │   上限前总分: {}  最终分: {}  应用上限: {}",
        r.confidence_breakdown.total_before_caps,
        r.confidence_breakdown.final_score,
        r.confidence_breakdown.applied_cap,
    );

    println!("  ├─ 价格上下文 ────────────────────────────────────────");
    let pc = &r.price_context;
    if let Some(p) = pc.current_price {
        println!("  │ 当前价: {:.2}", p);
    }
    if let (Some(h), Some(l)) = (pc.high_price, pc.low_price) {
        println!("  │ {}日高: {:.2} ({})  低: {:.2} ({})", pc.lookback_days, h, pc.high_date, l, pc.low_date);
    }
    if let Some(d) = pc.distance_to_high_pct {
        println!("  │ 距高点: {:.1}%", d);
    }
    if let Some(d) = pc.distance_to_low_pct {
        println!("  │ 距低点: {:.1}%", d);
    }

    println!("  ├─ 概率视角 ──────────────────────────────────────────");
    let pv = &r.probability_view;
    println!("  │ 上行概率: {:.0}%  下行概率: {:.0}%  横盘概率: {:.0}%  风险概率: {:.0}%",
        pv.upside_probability_pct, pv.downside_probability_pct,
        pv.sideways_probability_pct, pv.risk_probability_pct,
    );

    println!("  ├─ 研究结论 ──────────────────────────────────────────");
    println!("  │ CoreResearchCall: {}", r.core_research_call.key);
    println!("  │ 推荐: {}  (原始LLM: {})", r.recommendation.key, r.raw_llm_recommendation);
    println!("  │ 执行边界完整: {}  交易设置质量: {}",
        r.execution_readiness.execution_boundary_complete,
        r.trade_setup_quality.label.key);

    println!("  ├─ 技术指标 ──────────────────────────────────────────");
    for cat in &r.technical_indicators.categories {
        for ind in &cat.indicators {
            if let Some(v) = ind.value {
                println!("  │   {}: {:.2}  [{}]", ind.key, v, ind.signal_code);
            }
        }
    }
    for conc in &r.technical_indicators.conclusions {
        println!("  │   结论: {} ({})", conc.key, conc.severity);
    }

    println!("  ├─ 组合决策 ──────────────────────────────────────────");
    let pd = &r.portfolio_decision;
    println!("  │ 评级: {:?}  确认位: {}  止损位: {}",
        pd.rating, pd.confirmation_level, pd.invalidation_level);

    let summary: String = pd.executive_summary.key.chars().take(200).collect();
    println!("  └─ 摘要 ──────────────────────────────────────────────");
    println!("    {}", summary);
    println!();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    unsafe {
        std::env::set_var("ANALYSIS_DEBUG_QUICK_ONLY", "1");
        std::env::set_var("REPORT_KLINE_LIMIT", "60");
    }

    // DeepSeek config
    let base_url = "https://api.deepseek.com/v1";
    let api_key = "sk-fdac03cbcae74632ac5d8a6e99e4e229";
    let model = "deepseek-chat";

    let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let llm = sa::llm::LlmClient::openai_compatible(http, base_url, api_key, model, 600);
    println!("✅ DeepSeek LLM client ready\n");

    let symbol = "600519.SH";
    let name = "贵州茅台";
    let market_type = "A股";

    println!("=== DeepSeek测试: {} ({}) ===\n", name, symbol);

    let data_dir = tempfile::tempdir()?;
    let analysis_store: Arc<dyn sa::AnalysisStore> = Arc::new(InMemoryAnalysisStore::new());
    let cache_store: Arc<dyn sa::CacheStore> = Arc::new(InMemoryCacheStore::new());
    let checkpoint_inner: Arc<dyn sa::CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
    let checkpoint_store = sa::checkpoint::TaskCheckpointStore::new(checkpoint_inner);
    let market_data = sa::MarketDataClient::new().await?;
    let memory_log = sa::memory::TradingMemoryLog::new(data_dir.path().to_str().unwrap(), 100)?;
    let telemetry = sa::telemetry::init_telemetry();

    let manager = sa::TaskManager::new(
        analysis_store,
        cache_store,
        Some(llm.clone()),
        Some(llm.clone()),
        market_data,
        data_dir.path().to_str().unwrap().to_string(),
        memory_log,
        checkpoint_store,
        sa::env_config::debate_rounds(),
        sa::env_config::risk_discuss_rounds(),
        telemetry,
    )
    .await?;

    let request = sa::SingleAnalysisRequest {
        symbol: Some(symbol.to_string()),
        stock_code: None,
        stock_name: Some(name.to_string()),
        parameters: Some(sa::AnalysisParameters {
            market_type: Some(market_type.to_string()),
            analysis_date: Some(chrono::Local::now().format("%Y-%m-%d").to_string()),
            ..Default::default()
        }),
        force_refresh: true,
    };

    let task_id = manager
        .create_task_and_run_blocking("", request, None)
        .await
        .expect("task creation should succeed");

    let task = wait_for_task(&manager, &task_id, std::time::Duration::from_secs(1800)).await;

    let result = if task.status == sa::TaskStatus::Completed {
        manager.analysis_store().load_result(&task_id).await.unwrap()
    } else {
        None
    };

    println!("\n{}", "=".repeat(70));
    println!("=== DeepSeek结果 ===");
    println!("{}", "=".repeat(70));
    if let Some(ref r) = result {
        print!("  推荐: {}  (原始LLM: {})\n", r.report.recommendation.key, r.report.raw_llm_recommendation);
        print!("  置信度: {}\n", r.report.confidence_score);
        print!("  tokens: {}  reqs: {}\n", r.artifacts.llm_token_usage.total_tokens, r.artifacts.llm_token_usage.total_requests);
        print_detailed_indicators(r);
    } else {
        println!("  ❌ 无结果: {:?}", task.error_message);
    }

    Ok(())
}

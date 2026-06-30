//! Quick start example — analyze a stock in ~20 lines.
//!
//! # Setup
//! ```bash
//! export ANTHROPIC_BASE_URL=https://api.anthropic.com
//! export ANTHROPIC_API_KEY=sk-ant-xxx
//! export ANTHROPIC_MODEL=claude-sonnet-4-20250514
//! ```
//!
//! # Run
//! ```bash
//! cargo run --release --example quick_start
//! ```

use std::sync::Arc;
use sa::{InMemoryAnalysisStore, InMemoryCacheStore, InMemoryCheckpointStore};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Create LLM client from env vars
    let llm = sa::llm::LlmClient::from_env()
        .expect("Set ANTHROPIC_BASE_URL + ANTHROPIC_API_KEY (or LLM_* vars)");

    // 2. Setup in-memory stores (use persistent stores in production)
    let data_dir = tempfile::tempdir()?;
    let manager = sa::TaskManager::new(
        Arc::new(InMemoryAnalysisStore::new()),
        Arc::new(InMemoryCacheStore::new()),
        Some(llm.clone()),
        Some(llm.clone()),
        sa::MarketDataClient::new().await?,
        data_dir.path().to_str().unwrap().to_string(),
        sa::memory::TradingMemoryLog::new(data_dir.path().to_str().unwrap(), 100)?,
        sa::checkpoint::TaskCheckpointStore::new(Arc::new(InMemoryCheckpointStore::new())),
        sa::env_config::debate_rounds(),
        sa::env_config::risk_discuss_rounds(),
        sa::telemetry::init_telemetry(),
    ).await?;

    // 3. Submit analysis request
    let request = sa::SingleAnalysisRequest {
        symbol: Some("600519.SH".to_string()),
        stock_code: None,
        stock_name: Some("贵州茅台".to_string()),
        parameters: Some(sa::AnalysisParameters {
            market_type: Some("A股".to_string()),
            analysis_date: Some(chrono::Local::now().format("%Y-%m-%d").to_string()),
            ..Default::default()
        }),
        force_refresh: true,
    };

    let task_id = manager.create_task_and_run_blocking("", request, None).await?;

    // 4. Wait for completion
    let task = manager.wait_for_task(
        &task_id,
        std::time::Duration::from_secs(1800),
        Some(&mut |t| print!("⏳ {:?}...\r", t.status)),
    ).await?;

    // 5. Print result
    println!();
    if task.status == sa::TaskStatus::Completed {
        if let Some(result) = manager.analysis_store().load_result(&task_id).await? {
            let r = &result.report;
            println!("=== {} ({}) ===", result.stock_name, result.symbol);
            println!("推荐: {}  置信度: {}  方向分: {}", r.recommendation.key, r.confidence_score, r.direction_score);
            println!("摘要: {}", r.summary.key);
            if let Some(rr) = r.profit_risk.reward_risk_ratio {
                println!("盈亏比: {:.2}", rr);
            }
            println!("\nTokens: {}", result.artifacts.llm_token_usage.total_tokens);
        }
    } else {
        println!("❌ 分析失败: {}", task.error_message.unwrap_or_default());
    }

    Ok(())
}

use std::io::Write;
use std::sync::Arc;

use sa::{InMemoryAnalysisStore, InMemoryCacheStore, InMemoryCheckpointStore};

fn load_llm_config() -> Option<(String, String, String, u64)> {
    let settings_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude")
        .join("settings.json");

    if let Ok(content) = std::fs::read_to_string(&settings_path) {
        if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(env) = settings.get("env") {
                let base_url = env
                    .get("ANTHROPIC_BASE_URL")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let api_key = env
                    .get("ANTHROPIC_AUTH_TOKEN")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let model = env
                    .get("ANTHROPIC_MODEL")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let timeout_ms = env
                    .get("API_TIMEOUT_MS")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(30000);
                if let (Some(b), Some(k), Some(m)) = (base_url, api_key, model) {
                    return Some((b, k, m, timeout_ms / 1000));
                }
            }
        }
    }

    let base_url = std::env::var("ANTHROPIC_BASE_URL")
        .or_else(|_| std::env::var("LLM_BASE_URL"))
        .ok()?;
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .or_else(|_| std::env::var("LLM_API_KEY"))
        .ok()?;
    let model = std::env::var("ANTHROPIC_MODEL")
        .or_else(|_| std::env::var("LLM_MODEL"))
        .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
    let timeout_ms: u64 = std::env::var("API_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30000);
    Some((base_url, api_key, model, timeout_ms / 1000))
}

fn setup_llm_client() -> Option<sa::llm::LlmClient> {
    let (base_url, api_key, model, timeout_secs) = load_llm_config()?;
    let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let timeout = timeout_secs.max(600);
    Some(sa::llm::LlmClient::anthropic(
        http, &base_url, &api_key, &model, timeout,
    ))
}

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    unsafe {
        std::env::set_var("ANALYSIS_DEBUG_QUICK_ONLY", "1");
        std::env::set_var("REPORT_KLINE_LIMIT", "60");
    }

    let Some(llm) = setup_llm_client() else {
        eprintln!("❌ LLM config not found. Set LLM_BASE_URL + LLM_API_KEY");
        std::process::exit(1);
    };
    println!("✅ LLM client ready\n");

    let symbol = "600519.SH";
    let name = "贵州茅台";
    let market_type = "A股";

    println!("=== 单股测试: {} ({}) ===", name, symbol);

    let data_dir = tempfile::tempdir()?;
    let analysis_store: Arc<dyn sa::AnalysisStore> = Arc::new(InMemoryAnalysisStore::new());
    let cache_store: Arc<dyn sa::CacheStore> = Arc::new(InMemoryCacheStore::new());
    let checkpoint_inner: Arc<dyn sa::CheckpointStore> =
        Arc::new(InMemoryCheckpointStore::new());
    let checkpoint_store = sa::checkpoint::TaskCheckpointStore::new(checkpoint_inner);
    let market_data = sa::MarketDataClient::new().await?;
    let memory_log =
        sa::memory::TradingMemoryLog::new(data_dir.path().to_str().unwrap(), 100)?;
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

    let task =
        wait_for_task(&manager, &task_id, std::time::Duration::from_secs(1800)).await;

    let result = if task.status == sa::TaskStatus::Completed {
        manager
            .analysis_store()
            .load_result(&task_id)
            .await
            .unwrap()
    } else {
        None
    };

    println!("\n=== 结果 ===");
    println!("状态: {:?}", task.status);
    if let Some(r) = result {
        println!("推荐: {}", r.report.recommendation.key);
        println!("置信度: {}", r.report.confidence_score);
        println!("摘要: {}", r.report.summary.key);
    } else {
        println!("无结果");
    }

    Ok(())
}

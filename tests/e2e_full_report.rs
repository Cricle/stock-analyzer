mod common;

use std::io::Write;
use std::sync::Arc;

use common::memory_stores::{InMemoryAnalysisStore, InMemoryCacheStore, InMemoryCheckpointStore};

/// LLM configuration loaded from Claude settings or environment variables.
struct LlmConfig {
    base_url: String,
    api_key: String,
    model: String,
    timeout_secs: u64,
}

/// Load LLM config from Claude settings file (~/.claude/settings.json).
/// Falls back to environment variables if settings file doesn't exist.
fn load_llm_config() -> Option<LlmConfig> {
    // Try to read from Claude settings file first
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

                if let (Some(base_url), Some(api_key), Some(model)) = (base_url, api_key, model) {
                    println!("Loaded LLM config from Claude settings:");
                    println!("  Base URL: {}", base_url);
                    println!("  Model: {}", model);
                    println!("  Timeout: {}ms", timeout_ms);
                    return Some(LlmConfig {
                        base_url,
                        api_key,
                        model,
                        timeout_secs: timeout_ms / 1000,
                    });
                }
            }
        }
    }

    // Fall back to environment variables
    let base_url =
        match std::env::var("ANTHROPIC_BASE_URL").or_else(|_| std::env::var("LLM_BASE_URL")) {
            Ok(v) => v,
            Err(_) => return None,
        };
    let api_key =
        match std::env::var("ANTHROPIC_AUTH_TOKEN").or_else(|_| std::env::var("LLM_API_KEY")) {
            Ok(v) => v,
            Err(_) => return None,
        };
    let model = std::env::var("ANTHROPIC_MODEL")
        .or_else(|_| std::env::var("LLM_MODEL"))
        .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
    let timeout_ms: u64 = std::env::var("API_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30000);

    println!("Loaded LLM config from environment variables:");
    println!("  Base URL: {}", base_url);
    println!("  Model: {}", model);

    Some(LlmConfig {
        base_url,
        api_key,
        model,
        timeout_secs: timeout_ms / 1000,
    })
}

/// Enable quick-only debug mode to speed up analysis (fewer LLM calls).
fn enable_debug_mode() {
    // SAFETY: called once at test startup before any threads read env vars
    unsafe {
        std::env::set_var("ANALYSIS_DEBUG_QUICK_ONLY", "1");
        // Limit candle data to avoid huge prompts
        std::env::set_var("REPORT_KLINE_LIMIT", "60");
    }
}

fn setup_llm_client() -> Option<sa_engine::llm::LlmClient> {
    let config = load_llm_config()?;
    let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    // Use 600s timeout — mimo-v2.5-pro with thinking tokens takes 200-400s per call
    let timeout = config.timeout_secs.max(600);
    Some(sa_engine::llm::LlmClient::anthropic(
        http,
        &config.base_url,
        &config.api_key,
        &config.model,
        timeout,
    ))
}

async fn setup_task_manager() -> Option<(sa_engine::TaskManager, tempfile::TempDir)> {
    enable_debug_mode();
    let data_dir = tempfile::tempdir().unwrap();
    let analysis_store: Arc<dyn sa_models::AnalysisStore> = Arc::new(InMemoryAnalysisStore::new());
    let cache_store: Arc<dyn sa_models::CacheStore> = Arc::new(InMemoryCacheStore::new());
    let checkpoint_inner: Arc<dyn sa_models::CheckpointStore> =
        Arc::new(InMemoryCheckpointStore::new());
    let checkpoint_store = sa_engine::checkpoint::TaskCheckpointStore::new(checkpoint_inner);
    let market_data = sa_data::MarketDataClient::new().await;
    let memory_log =
        sa_engine::memory::TradingMemoryLog::new(data_dir.path().to_str().unwrap(), 100).unwrap();
    let telemetry = sa_engine::telemetry::init_telemetry();
    let llm = setup_llm_client()?;

    let manager = sa_engine::TaskManager::new(
        analysis_store,
        cache_store,
        Some(llm.clone()),
        Some(llm),
        market_data,
        data_dir.path().to_str().unwrap().to_string(),
        memory_log,
        checkpoint_store,
        1,
        1,
        telemetry,
    )
    .await
    .unwrap();

    Some((manager, data_dir))
}

/// Poll until a task reaches a terminal status (Completed or Failed).
async fn wait_for_task(
    manager: &sa_engine::TaskManager,
    task_id: &str,
    timeout: std::time::Duration,
) -> sa_models::PersistedTask {
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
            sa_models::TaskStatus::Completed | sa_models::TaskStatus::Failed => {
                println!(
                    "[{:?}] Task {} finished: {:?}",
                    start.elapsed(),
                    task_id,
                    task.status
                );
                if let Some(ref err) = task.error_message {
                    eprintln!("ERROR MESSAGE: {}", err);
                }
                let _ = std::io::stdout().flush();
                let _ = std::io::stderr().flush();
                return task;
            }
            _ => {
                println!(
                    "[{:?}] Task {} progress: {}% - {} ({})",
                    start.elapsed(),
                    task_id,
                    task.progress,
                    task.current_step_name,
                    task.current_step_description
                );
                let _ = std::io::stdout().flush();
            }
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "task {} did not complete within {:?}, last status: {:?}, progress: {}%, step: {}",
                task_id, timeout, task.status, task.progress, task.current_step_name
            );
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

#[tokio::test]
async fn e2e_full_report_aapl() {
    let Some((manager, _data_dir)) = setup_task_manager().await else {
        eprintln!("Skipping: LLM config not available");
        return;
    };
    let request = sa_models::SingleAnalysisRequest {
        symbol: Some("AAPL".to_string()),
        stock_code: None,
        stock_name: Some("Apple".to_string()),
        parameters: Some(sa_models::AnalysisParameters {
            market_type: Some("US".to_string()),
            analysis_date: Some(chrono::Local::now().format("%Y-%m-%d").to_string()),
            ..Default::default()
        }),
        force_refresh: true,
    };

    let task_id = manager
        .create_task_and_run_blocking("", request, None)
        .await
        .expect("task creation should succeed");

    println!("Task created: {}, waiting for completion...", task_id);

    let task = wait_for_task(&manager, &task_id, std::time::Duration::from_secs(3600)).await;

    println!("Task status: {:?}", task.status);
    println!("LLM tokens: {}", task.llm_token_usage.total_tokens);
    if let Some(ref err) = task.error_message {
        eprintln!("Task error: {}", err);
    }
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    assert_eq!(
        task.status,
        sa_models::TaskStatus::Completed,
        "task should complete successfully"
    );

    let result = manager
        .analysis_store()
        .load_result(&task_id)
        .await
        .unwrap();
    let result = result.expect("completed task should have a result");

    let summary = &result.report.summary;
    let s = summary.as_str();
    let end = {
        let mut e = 200.min(s.len());
        while e > 0 && !s.is_char_boundary(e) {
            e -= 1;
        }
        e
    };
    println!("Summary: {}", &s[..end]);
    println!("Recommendation: {}", result.report.recommendation);
    assert!(!summary.is_empty(), "summary should not be empty");
}

/// Run a single stock report and validate the result.
async fn run_single_stock(symbol: &str, name: &str, market: &str) {
    run_single_stock_owned(symbol.to_string(), name.to_string(), market.to_string()).await
}

async fn run_single_stock_owned(symbol: String, name: String, market: String) {
    let Some((manager, _data_dir)) = setup_task_manager().await else {
        eprintln!("Skipping {}: LLM config not available", symbol);
        return;
    };
    println!("\n=== Running report for {} ({}) ===", name, symbol);
    let request = sa_models::SingleAnalysisRequest {
        symbol: Some(symbol.to_string()),
        stock_code: None,
        stock_name: Some(name.to_string()),
        parameters: Some(sa_models::AnalysisParameters {
            market_type: Some(market.to_string()),
            analysis_date: Some(chrono::Local::now().format("%Y-%m-%d").to_string()),
            selected_analysts: Some(vec!["market".to_string()]),
            ..Default::default()
        }),
        force_refresh: true,
    };

    let task_id = manager
        .create_task_and_run_blocking("", request, None)
        .await
        .expect("task creation should succeed");

    let task = wait_for_task(&manager, &task_id, std::time::Duration::from_secs(3600)).await;

    let result = manager
        .analysis_store()
        .load_result(&task_id)
        .await
        .unwrap();
    let result = result.expect("completed task should have a result");

    let summary = &result.report.summary;
    let s = summary.as_str();
    let end = {
        let mut e = 200.min(s.len());
        while e > 0 && !s.is_char_boundary(e) {
            e -= 1;
        }
        e
    };
    println!("Summary: {}", &s[..end]);
    println!("Recommendation: {}", result.report.recommendation);
    assert_eq!(task.status, sa_models::TaskStatus::Completed);
    assert!(!summary.is_empty(), "summary should not be empty");
}

#[tokio::test]
async fn e2e_full_report_tencent() {
    run_single_stock(
        "00700",
        "\u{817e}\u{8baf}\u{63a7}\u{80a1}",
        "\u{6e2f}\u{80a1}",
    )
    .await;
}

#[tokio::test]
async fn e2e_full_report_sensetime() {
    run_single_stock(
        "00020",
        "\u{5546}\u{6c64}\u{79d1}\u{6280}",
        "\u{6e2f}\u{80a1}",
    )
    .await;
}

#[tokio::test]
async fn e2e_full_report_pltr() {
    run_single_stock("PLTR", "Palantir", "\u{7f8e}\u{80a1}").await;
}

/// Run all 6 stocks with concurrency limit of 2 (memory-safe for 3.8GB RAM).
#[tokio::test]
async fn e2e_full_report_all_parallel() {
    let stocks: Vec<(String, String, String)> = vec![
        (
            "600519".into(),
            "\u{8d35}\u{5dde}\u{8305}\u{53f0}".into(),
            "A\u{80a1}".into(),
        ),
        (
            "688256".into(),
            "\u{5bd2}\u{6b66}\u{7eaa}".into(),
            "A\u{80a1}".into(),
        ),
        (
            "00700".into(),
            "\u{817e}\u{8baf}\u{63a7}\u{80a1}".into(),
            "\u{6e2f}\u{80a1}".into(),
        ),
        (
            "00020".into(),
            "\u{5546}\u{6c64}\u{79d1}\u{6280}".into(),
            "\u{6e2f}\u{80a1}".into(),
        ),
        ("AAPL".into(), "Apple".into(), "\u{7f8e}\u{80a1}".into()),
        ("PLTR".into(), "Palantir".into(), "\u{7f8e}\u{80a1}".into()),
    ];

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
    let mut handles = Vec::new();

    for (symbol, name, market) in stocks {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let s = symbol.clone();
        let n = name.clone();
        let m = market.clone();
        let display_sym = symbol.clone();
        handles.push(tokio::spawn(async move {
            let result = tokio::task::spawn(run_single_stock_owned(s, n, m)).await;
            drop(permit);
            (display_sym, result)
        }));
    }

    let mut ok_count = 0;
    for h in handles {
        let (symbol, result) = h.await.unwrap();
        match result {
            Ok(()) => {
                println!("  {}: OK", symbol);
                ok_count += 1;
            }
            Err(e) => {
                println!("  {}: FAILED - {:?}", symbol, e);
            }
        }
    }

    println!("\n=== Results: {}/6 passed ===", ok_count);
    assert!(ok_count >= 4, "Expected at least 4/6, got {}", ok_count);
}

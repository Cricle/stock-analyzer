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

fn print_result(label: &str, task: &sa::PersistedTask, result: &Option<sa::AnalysisResult>) {
    print!("  {:<20} {:<10}", label, format!("{:?}", task.status));
    if let Some(r) = result {
        let rec = &r.report.recommendation.key;
        let conf = r.report.confidence_score;
        let summary = &r.report.summary.key;
        let tokens = r.artifacts.llm_token_usage.total_tokens;
        let requests = r.artifacts.llm_token_usage.total_requests;
        print!("  rec={:<12} conf={:<3} tokens={:<8} reqs={:<3}", rec, conf, tokens, requests);
        let truncated: String = summary.chars().take(60).collect();
        print!("  {}", truncated);
    }
    println!();
}

fn print_detailed_indicators(result: &sa::AnalysisResult) {
    let r = &result.report;

    // ── Core Scores ──
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
    println!("  │   一致性: {}  执行水平: {}  仓位纪律: {}  视野清晰: {}  盈亏比: {}",
        r.action_breakdown.alignment.score,
        r.action_breakdown.execution_levels.score,
        r.action_breakdown.sizing_discipline.score,
        r.action_breakdown.horizon_clarity.score,
        r.action_breakdown.reward_to_risk.score,
    );
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
    println!("  │ 研究可靠度: {}/{}  ({})", r.research_reliability.score, r.research_reliability.max_score, r.research_reliability.label.key);

    // ── Price Context ──
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

    // ── Probability View ──
    println!("  ├─ 概率视角 ──────────────────────────────────────────");
    let pv = &r.probability_view;
    println!("  │ 上行概率: {:.0}%  下行概率: {:.0}%  横盘概率: {:.0}%  风险概率: {:.0}%",
        pv.upside_probability_pct, pv.downside_probability_pct,
        pv.sideways_probability_pct, pv.risk_probability_pct,
    );
    if let Some(t) = pv.upside_target {
        print!("  │ 上行目标: {:.2}", t);
        if let Some(pct) = pv.upside_pct {
            print!("  ({:+.1}%)", pct);
        }
        println!();
    }
    if let Some(t) = pv.downside_target {
        print!("  │ 下行目标: {:.2}", t);
        if let Some(pct) = pv.downside_pct {
            print!("  ({:+.1}%)", pct);
        }
        println!();
    }

    // ── Profit/Risk ──
    println!("  ├─ 盈亏比 ────────────────────────────────────────────");
    let pr = &r.profit_risk;
    if let Some(rr) = pr.reward_risk_ratio {
        println!("  │ 盈亏比: {:.2}", rr);
    }
    if let Some(rr) = pr.current_position_reward_risk_ratio {
        println!("  │ 当前仓位盈亏比: {:.2}", rr);
    }

    // ── Core Research Call ──
    println!("  ├─ 研究结论 ──────────────────────────────────────────");
    println!("  │ CoreResearchCall: {}", r.core_research_call.key);
    println!("  │ 推荐: {}  (原始LLM: {})", r.recommendation.key, r.raw_llm_recommendation);
    println!("  │ 执行边界完整: {}  强制持有: {}  交易设置质量: {}",
        r.execution_readiness.execution_boundary_complete,
        r.execution_readiness.forced_hold,
        r.trade_setup_quality.label.key);

    // ── Technical Indicators ──
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

    // ── IC Discipline ──
    let ic = &r.ic_discipline;
    if !ic.state.key.is_empty() {
        println!("  ├─ IC纪律 ────────────────────────────────────────────");
        println!("  │ 状态: {}", ic.state.key);
        if let Some(p) = ic.current_price {
            print!("  │ 当前: {:.2}", p);
            if let Some(c) = ic.confirmation_price {
                print!("  确认位: {:.2}", c);
            }
            if let Some(inv) = ic.invalidation_price {
                print!("  止损位: {:.2}", inv);
            }
            println!();
        }
        if let Some(rsi) = ic.rsi {
            print!("  │ RSI: {:.1}", rsi);
            if let Some(macd) = ic.macd {
                print!("  MACD: {:.4}", macd);
            }
            println!();
        }
        println!("  │ 上行概率: {:.0}%  下行概率: {:.0}%  风险概率: {:.0}%",
            ic.upside_probability_pct, ic.downside_probability_pct, ic.risk_probability_pct);
    }

    // ── Portfolio Decision ──
    println!("  ├─ 组合决策 ──────────────────────────────────────────");
    let pd = &r.portfolio_decision;
    println!("  │ 评级: {:?}  确认位: {}  止损位: {}",
        pd.rating, pd.confirmation_level, pd.invalidation_level);
    println!("  │ 目标参考: {}", pd.target_reference);

    // ── Executive Summary ──
    println!("  └─ 摘要 ──────────────────────────────────────────────");
    let summary: String = pd.executive_summary.key.chars().take(200).collect();
    println!("    {}", summary);
    println!();
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

    let markets: Vec<(&str, &str, Vec<(&str, &str)>)> = vec![
        (
            "A股",
            "A股",
            vec![
                ("600519.SH", "贵州茅台"),
                ("601318.SH", "中国平安"),
                ("000858.SZ", "五粮液"),
                ("300750.SZ", "宁德时代"),
                ("600036.SH", "招商银行"),
                ("000333.SZ", "美的集团"),
                ("601012.SH", "隆基绿能"),
                ("002594.SZ", "比亚迪"),
            ],
        ),
        (
            "美股",
            "美股",
            vec![
                ("AAPL", "Apple"),
                ("MSFT", "Microsoft"),
                ("NVDA", "NVIDIA"),
                ("GOOGL", "Alphabet"),
            ],
        ),
        (
            "港股",
            "港股",
            vec![
                ("0700.HK", "腾讯控股"),
                ("9988.HK", "阿里巴巴"),
                ("1810.HK", "小米集团"),
                ("3690.HK", "美团"),
            ],
        ),
    ];

    println!("=== 市场报告测试 ({} 只股票) ===", markets.iter().map(|(_, _, s)| s.len()).sum::<usize>());
    println!("模式: debug_quick_only, K线: 60根\n");

    for (market_label, market_type, stocks) in &markets {
        println!("\n{}", "=".repeat(70));
        println!("📈 {} — {} 只股票", market_label, stocks.len());
        println!("{}", "=".repeat(70));

        for (symbol, name) in stocks {
            println!("\n--- {} ({}) ---", name, symbol);

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

            print_result(&format!("{}/{}", symbol, name), &task, &result);
            if let Some(ref r) = result {
                print_detailed_indicators(r);
            }
        }
    }

    println!("\n{}", "=".repeat(70));
    println!("=== 完成 ===");
    Ok(())
}

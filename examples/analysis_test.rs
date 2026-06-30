//! Unified stock analysis example.
//!
//! Supports Anthropic (Claude) and DeepSeek. Tests A股/美股/港股 with parallel execution.
//!
//! # Quick Start
//!
//! ```bash
//! # Anthropic
//! export ANTHROPIC_BASE_URL=https://api.anthropic.com
//! export ANTHROPIC_API_KEY=sk-ant-xxx
//! export ANTHROPIC_MODEL=claude-sonnet-4-20250514
//!
//! # Or DeepSeek
//! export LLM_PROVIDER=deepseek
//! export LLM_BASE_URL=https://api.deepseek.com/v1
//! export LLM_API_KEY=sk-xxx
//!
//! # Run
//! cargo run --release --example analysis_test
//! ```

use std::sync::Arc;

use sa::{InMemoryAnalysisStore, InMemoryCacheStore, InMemoryCheckpointStore};

async fn run_single_stock(
    llm: &sa::llm::LlmClient,
    symbol: &str,
    name: &str,
    market_type: &str,
) -> anyhow::Result<(String, sa::PersistedTask, Option<sa::AnalysisResult>)> {
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

    let task_id = manager.create_task_and_run_blocking("", request, None).await?;
    let task = manager
        .wait_for_task(&task_id, std::time::Duration::from_secs(1800))
        .await?;

    let result = if task.status == sa::TaskStatus::Completed {
        manager.analysis_store().load_result(&task_id).await?
    } else {
        None
    };

    Ok((format!("{}/{}", symbol, name), task, result))
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

fn print_detailed(result: &sa::AnalysisResult) {
    let r = &result.report;

    println!("  ┌─ 核心评分 ──────────────────────────────────────────");
    println!("  │ 方向分: {}  (市场:{} 基本面:{} 舆情:{} 情绪:{})",
        r.direction_score,
        r.direction_breakdown.market.score,
        r.direction_breakdown.fundamentals.score,
        r.direction_breakdown.news.score,
        r.direction_breakdown.sentiment.score);
    println!("  │ 行动分: {}  (一致性:{} 执行:{} 仓位:{} 盈亏比:{})",
        r.action_score,
        r.action_breakdown.alignment.score,
        r.action_breakdown.execution_levels.score,
        r.action_breakdown.sizing_discipline.score,
        r.action_breakdown.reward_to_risk.score);
    println!("  │ 置信度: {}  (数据:{} 趋势:{} 基本面:{} 催化:{})",
        r.confidence_score,
        r.confidence_breakdown.data_quality.score,
        r.confidence_breakdown.trend_confirmation.score,
        r.confidence_breakdown.fundamental_confirmation.score,
        r.confidence_breakdown.catalyst_quality.score);
    println!("  │ 研究可靠: {}/{}", r.research_reliability.score, r.research_reliability.max_score);

    // Price
    let pc = &r.price_context;
    if let Some(p) = pc.current_price {
        println!("  ├─ 价格 ──────────────────────────────────────────────");
        println!("  │ 当前: {:.2}", p);
        if let (Some(h), Some(l)) = (pc.high_price, pc.low_price) {
            println!("  │ {}日高: {:.2}  低: {:.2}", pc.lookback_days, h, l);
        }
    }

    // Probability
    let pv = &r.probability_view;
    println!("  ├─ 概率 ──────────────────────────────────────────────");
    println!("  │ 上行:{:.0}%  下行:{:.0}%  横盘:{:.0}%",
        pv.upside_probability_pct, pv.downside_probability_pct, pv.sideways_probability_pct);

    // Decision
    let pd = &r.portfolio_decision;
    println!("  ├─ 决策 ──────────────────────────────────────────────");
    println!("  │ 推荐: {}  (LLM:{} → 校准:{})",
        r.recommendation.key, r.raw_llm_recommendation, pd.calibrated_rating);
    println!("  │ 确认位:{}  止损位:{}  目标:{}", pd.confirmation_level, pd.invalidation_level, pd.target_reference);
    if let Some(rr) = r.profit_risk.reward_risk_ratio {
        println!("  │ 盈亏比: {:.2}", rr);
    }

    // Technical
    if !r.technical_indicators.categories.is_empty() {
        println!("  ├─ 技术 ──────────────────────────────────────────────");
        for cat in &r.technical_indicators.categories {
            for ind in &cat.indicators {
                if let Some(v) = ind.value {
                    println!("  │   {}: {:.2}  [{}]", ind.key, v, ind.signal_code);
                }
            }
        }
    }

    // Summary
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

    let llm = sa::llm::LlmClient::from_env()
        .expect("Set ANTHROPIC_BASE_URL + ANTHROPIC_API_KEY (or LLM_* vars)");
    println!("✅ LLM client ready (model: {})\n", llm.model);

    let markets: Vec<(&str, &str, Vec<(&str, &str)>)> = vec![
        ("A股", "A股", vec![("600519.SH", "贵州茅台"), ("002709.SZ", "天赐材料")]),
        ("美股", "美股", vec![("AAPL", "Apple"), ("ENPH", "Enphase Energy")]),
        ("港股", "港股", vec![("0700.HK", "腾讯控股"), ("2015.HK", "理想汽车")]),
    ];

    let total: usize = markets.iter().map(|(_, _, s)| s.len()).sum();
    println!("=== 市场报告测试 ({} 只股票) ===\n", total);

    for (label, market_type, stocks) in &markets {
        println!("{}", "=".repeat(70));
        println!("📈 {} — {} 只 (并行)", label, stocks.len());
        println!("{}", "=".repeat(70));

        let mut handles = Vec::new();
        for (symbol, name) in stocks {
            let llm = llm.clone();
            let (symbol, name, market) = (symbol.to_string(), name.to_string(), market_type.to_string());
            handles.push(tokio::spawn(async move {
                run_single_stock(&llm, &symbol, &name, &market).await
            }));
        }

        for handle in handles {
            match handle.await? {
                Ok((label, task, result)) => {
                    println!("\n--- {} ---", label);
                    print_result(&label, &task, &result);
                    if let Some(ref r) = result {
                        print_detailed(r);
                    }
                }
                Err(e) => eprintln!("  ❌ {}", e),
            }
        }
    }

    println!("=== 完成 ===");
    Ok(())
}

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
    } else if task.status == sa::TaskStatus::Failed {
        if let Some(ref err) = task.error_message {
            if !err.is_empty() {
                print!("  error: {}", err);
            } else {
                print!("  (no error details)");
            }
        } else {
            print!("  (no error details)");
        }
    }
    println!();
}

fn print_detailed(result: &sa::AnalysisResult) {
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
    println!("  │ 推荐: {}  (LLM: {} → 校准: {})",
        r.recommendation.key,
        r.raw_llm_recommendation,
        r.portfolio_decision.calibrated_rating);
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

    // Show "how to break Hold" when recommendation is Hold
    if pd.rating == sa::Rating::Hold {
        println!("  ├─ 如何打破Hold ─────────────────────────────────────");
        if !pd.trigger_checklist.is_empty() {
            println!("  │ 升级条件:");
            for (i, trigger) in pd.trigger_checklist.iter().enumerate() {
                println!("  │   {}. {}", i + 1, trigger);
            }
        }
        if !pd.confirmation_level.is_empty() {
            println!("  │ 确认位: {}", pd.confirmation_level);
        }
        let dv = &r.decision_view;
        if !dv.next_upgrade_condition.key.is_empty() {
            println!("  │ 下一步升级: {}", dv.next_upgrade_condition.key);
        }
        let direction = r.direction_score;
        let confidence = r.confidence_score;
        println!("  │ 原因: direction_score={} (需要>=50才能覆盖), confidence={}",
            direction, confidence);
    }

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

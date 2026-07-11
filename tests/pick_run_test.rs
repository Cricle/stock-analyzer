use sa::MarketDataClient;
use sa::StockPickRequest;
use sa::llm::LlmClient;

#[tokio::test]
#[ignore]
async fn pick_a_share() {
    run_pick("A股", "A-share").await;
}

#[tokio::test]
#[ignore]
async fn pick_us() {
    run_pick("美股", "US").await;
}

#[tokio::test]
#[ignore]
async fn pick_hk() {
    run_pick("港股", "HK").await;
}

async fn run_pick(market: &str, market_label: &str) {
    let llm = match LlmClient::from_env() {
        Some(client) => client,
        None => {
            eprintln!("Skipping {}: LLM config not available", market);
            return;
        }
    };
    let market_data = match MarketDataClient::new().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping {}: MarketDataClient init failed: {}", market, e);
            return;
        }
    };

    println!("\n=== 选股: {} ===", market);
    let request = StockPickRequest {
        market: market_label.to_string(),
        analysis_date: Some(chrono::Local::now().format("%Y-%m-%d").to_string()),
        strategy: Some("balanced swing selection".to_string()),
        candidate_limit: Some(10),
        pick_count: Some(3),
        sector_type: None,
        candidate_symbols: None,
        language: None,
        target_output_mode: None,
        search_depth: None,
        history_retrieval: None,
    };

    match sa::pick::run(&market_data, &llm, &request).await {
        Ok(response) => {
            println!("  摘要: {}", response.summary);
            println!("  选出 {} 只:", response.picks.len());
            for (i, pick) in response.picks.iter().enumerate() {
                println!(
                    "  {}. {} | 置信度: {} | 论点: {}",
                    i + 1,
                    pick.symbol,
                    pick.confidence,
                    pick.thesis.key.chars().take(80).collect::<String>()
                );
                if !pick.catalysts.is_empty() {
                    let cats: Vec<&str> = pick.catalysts.iter().map(|c| c.key.as_str()).collect();
                    println!("     催化剂: {}", cats.join(", "));
                }
                if !pick.risks.is_empty() {
                    let rks: Vec<&str> = pick.risks.iter().map(|r| r.key.as_str()).collect();
                    println!("     风险: {}", rks.join(", "));
                }
            }
            if !response.rejected_symbols.is_empty() {
                println!("  淘汰: {}", response.rejected_symbols.join(", "));
            }
        }
        Err(e) => {
            println!("  失败: {}", e);
        }
    }
}

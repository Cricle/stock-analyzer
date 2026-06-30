use sa::llm::LlmClient;
use sa::data::MarketDataClient;
use sa::StockPickRequest;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sa=info".parse().unwrap()),
        )
        .init();

    let llm = match LlmClient::from_env() {
        Some(client) => client,
        None => {
            eprintln!("LLM config not available");
            return Ok(());
        }
    };
    let market_data = MarketDataClient::new().await?;

    let markets = [
        ("A股", "A-share"),
        ("港股", "HK"),
        ("美股", "US"),
    ];

    for (market_name, market_label) in markets {
        println!("\n=== 选股: {} ===", market_name);

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
                    println!("  {}. {} | 置信度: {} | 论点: {}",
                        i + 1, pick.symbol, pick.confidence,
                        pick.thesis.chars().take(80).collect::<String>());
                    if !pick.catalysts.is_empty() {
                        println!("     催化剂: {}", pick.catalysts.join(", "));
                    }
                    if !pick.risks.is_empty() {
                        println!("     风险: {}", pick.risks.join(", "));
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

    Ok(())
}

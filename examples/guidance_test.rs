use sa::data::MarketDataClient;
use sa::guide::{
    DailyGuidanceGenerator, DailyGuidanceRequest, GuidanceMemory,
    GuidanceMemoryBundle,
};

struct NoopGuidanceMemory;

#[async_trait::async_trait]
impl GuidanceMemory for NoopGuidanceMemory {
    async fn past_context_bundle(
        &self,
        _query: &str,
        _same_ticker_limit: usize,
        _cross_ticker_limit: usize,
    ) -> GuidanceMemoryBundle {
        GuidanceMemoryBundle::default()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with env filter
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sa=debug".parse().unwrap())
                .add_directive("akshare=debug".parse().unwrap()),
        )
        .init();

    let http = reqwest::Client::new();
    let market_data = MarketDataClient::new().await?;
    let memory = std::sync::Arc::new(NoopGuidanceMemory);
    let generator = DailyGuidanceGenerator::new(market_data, memory, http);

    let markets = [
        ("a_share", "A股"),
        ("hong_kong", "港股"),
        ("us_equity", "美股"),
    ];

    for (market_key, market_name) in markets {
        println!("\n=== 指引: {} ===", market_name);

        let request = DailyGuidanceRequest {
            market: Some(market_key.to_string()),
            tickers: None,
            refresh: Some(true),
        };

        match generator.generate(&request).await {
            Ok(report) => {
                println!("  生成成功!");
                println!(
                    "  市场情绪: {} ({})",
                    report.market_sentiment.label, report.market_sentiment.score
                );
                println!("  新闻数量: {}", report.key_news.len());
                println!("  板块亮点: {}", report.sector_highlights.len());
                println!("  风险提示: {}", report.risk_alerts.len());
                println!("  生成耗时: {}ms", report.metadata.generation_time_ms);

                if !report.key_news.is_empty() {
                    println!("  最新新闻:");
                    for (i, news) in report.key_news.iter().take(3).enumerate() {
                        println!("    {}. {} [{}]", i + 1, news.title, news.impact);
                    }
                }
            }
            Err(e) => {
                println!("  生成失败: {}", e);
            }
        }
    }

    Ok(())
}

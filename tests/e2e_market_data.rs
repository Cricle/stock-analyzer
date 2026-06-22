mod common;

use common::stocks::TEST_STOCKS;
use common::eval::{
    StockEvalResult, assert_quote_valid, assert_fundamentals_valid,
    assert_news_valid, assert_candles_valid, print_completeness_table,
};

#[tokio::test]
async fn e2e_fetch_all_market_data() {
    let client = sa_data::MarketDataClient::new().await;
    let mut results = Vec::new();

    for stock in TEST_STOCKS {
        let symbol = stock.symbol;
        println!("\n=== Fetching {} ({}) ===", stock.name, symbol);

        // Quote
        let quote_result = client.fetch_quote(symbol).await;
        let quote_ok = match &quote_result {
            Ok(q) => {
                let valid = assert_quote_valid(q);
                println!("  Quote: price={} volume={} valid={}",
                    q.close, q.volume, valid);
                valid
            }
            Err(e) => {
                println!("  Quote: ERROR - {}", e);
                false
            }
        };

        // Fundamentals
        let fund_result = client.fetch_fundamentals(symbol).await;
        let (fundamentals_ok, fundamentals_partial) = match &fund_result {
            Ok(f) => {
                let (complete, partial) = assert_fundamentals_valid(f);
                println!("  Fundamentals: name={} currency={} market_cap={:?} complete={} partial={}",
                    f.company_name, f.currency, f.market_cap, complete, partial);
                (complete, partial)
            }
            Err(e) => {
                println!("  Fundamentals: ERROR - {}", e);
                (false, false)
            }
        };

        // News (30-day window)
        let news_result = client.fetch_news(symbol, 20, None, None).await;
        let (news_ok, news_count) = match &news_result {
            Ok(items) => {
                let valid = assert_news_valid(items);
                println!("  News: {} items, valid={}", items.len(), valid);
                for (i, item) in items.iter().take(3).enumerate() {
                    println!("    [{}] {} - {}", i + 1, item.published_at, item.title);
                }
                (valid, items.len())
            }
            Err(e) => {
                println!("  News: ERROR - {}", e);
                (false, 0)
            }
        };

        // Candles (120 days to ensure enough data)
        let candles_result = client.fetch_candles(symbol, "qfq", 120).await;
        let (candles_ok, candle_count) = match &candles_result {
            Ok(candles) => {
                let valid = assert_candles_valid(candles);
                println!("  Candles: {} days, valid={}", candles.len(), valid);
                if let Some(last) = candles.last() {
                    println!("    Last: {} O={} H={} L={} C={} V={}",
                        last.trade_date, last.open, last.high, last.low, last.close, last.volume);
                }
                (valid, candles.len())
            }
            Err(e) => {
                println!("  Candles: ERROR - {}", e);
                (false, 0)
            }
        };

        results.push(StockEvalResult {
            symbol: symbol.to_string(),
            name: stock.name.to_string(),
            quote_ok,
            fundamentals_ok,
            fundamentals_partial,
            news_ok,
            news_count,
            candles_ok,
            candle_count,
        });
    }

    print_completeness_table(&results);

    // Assertions based on success criteria
    let quotes_ok = results.iter().filter(|r| r.quote_ok).count();
    assert!(quotes_ok >= 4, "Expected at least 4/6 stocks with valid quotes, got {}", quotes_ok);

    let funds_ok = results.iter().filter(|r| r.fundamentals_ok || r.fundamentals_partial).count();
    assert!(funds_ok >= 4, "Expected at least 4/6 stocks with fundamentals, got {}", funds_ok);

    let news_ok = results.iter().filter(|r| r.news_ok).count();
    assert!(news_ok >= 3, "Expected at least 3/6 stocks with valid news, got {}", news_ok);
}

#[tokio::test]
async fn e2e_market_detection() {
    let client = sa_data::MarketDataClient::new().await;

    for stock in TEST_STOCKS {
        let detected = client.detect_market(stock.symbol);
        assert_eq!(detected, stock.market_kind,
            "Market detection for {} should be {:?}, got {:?}",
            stock.symbol, stock.market_kind, detected);
    }
}

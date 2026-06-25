use sa::guide::{PrewarmTask, generate_prewarm_tasks};

#[test]
fn generate_prewarm_tasks_returns_three_markets() {
    let tasks = generate_prewarm_tasks();
    assert_eq!(tasks.len(), 3);
}

#[test]
fn generate_prewarm_tasks_markets() {
    let tasks = generate_prewarm_tasks();
    let markets: Vec<&str> = tasks.iter().map(|t| t.market.as_str()).collect();
    assert!(markets.contains(&"a_share"));
    assert!(markets.contains(&"hong_kong"));
    assert!(markets.contains(&"us_equity"));
}

#[test]
fn generate_prewarm_tasks_ticker_counts() {
    let tasks = generate_prewarm_tasks();
    for task in &tasks {
        assert_eq!(
            task.tickers.len(),
            5,
            "{} should have 5 tickers",
            task.market
        );
    }
}

#[test]
fn generate_prewarm_tasks_date_is_today() {
    let tasks = generate_prewarm_tasks();
    let today = chrono::Utc::now().date_naive().to_string();
    for task in &tasks {
        assert_eq!(task.date, today);
    }
}

#[test]
fn generate_prewarm_tasks_a_share_tickers() {
    let tasks = generate_prewarm_tasks();
    let a_share = tasks.iter().find(|t| t.market == "a_share").unwrap();
    assert!(a_share.tickers.contains(&"600519".to_string()));
    assert!(a_share.tickers.contains(&"000001".to_string()));
}

#[test]
fn generate_prewarm_tasks_us_tickers() {
    let tasks = generate_prewarm_tasks();
    let us = tasks.iter().find(|t| t.market == "us_equity").unwrap();
    assert!(us.tickers.contains(&"AAPL".to_string()));
    assert!(us.tickers.contains(&"NVDA".to_string()));
}

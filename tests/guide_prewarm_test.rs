use stock_analyzer::guide::generate_prewarm_tasks;

#[test]
fn generate_prewarm_tasks_with_custom_tickers() {
    let tasks = generate_prewarm_tasks(&[
        ("a_share", vec!["000001".to_string(), "600036".to_string()]),
        ("us_equity", vec!["AAPL".to_string(), "MSFT".to_string()]),
    ]);
    assert_eq!(tasks.len(), 2);
}

#[test]
fn generate_prewarm_tasks_markets() {
    let tasks = generate_prewarm_tasks(&[
        ("a_share", vec!["000001".to_string()]),
        ("hong_kong", vec!["00700".to_string()]),
        ("us_equity", vec!["AAPL".to_string()]),
    ]);
    let markets: Vec<&str> = tasks.iter().map(|t| t.market.as_str()).collect();
    assert!(markets.contains(&"a_share"));
    assert!(markets.contains(&"hong_kong"));
    assert!(markets.contains(&"us_equity"));
}

#[test]
fn generate_prewarm_tasks_preserves_tickers() {
    let tasks =
        generate_prewarm_tasks(&[("a_share", vec!["000001".to_string(), "600036".to_string()])]);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].tickers.len(), 2);
    assert!(tasks[0].tickers.contains(&"000001".to_string()));
}

#[test]
fn generate_prewarm_tasks_date_is_today() {
    let tasks = generate_prewarm_tasks(&[("a_share", vec!["000001".to_string()])]);
    let today = chrono::Utc::now().date_naive().to_string();
    for task in &tasks {
        assert_eq!(task.date, today);
    }
}

#[test]
fn generate_prewarm_tasks_skips_empty_tickers() {
    let tasks = generate_prewarm_tasks(&[
        ("a_share", vec!["000001".to_string()]),
        ("hong_kong", vec![]),
        ("us_equity", vec!["AAPL".to_string()]),
    ]);
    assert_eq!(tasks.len(), 2);
    assert!(tasks.iter().all(|t| !t.tickers.is_empty()));
}

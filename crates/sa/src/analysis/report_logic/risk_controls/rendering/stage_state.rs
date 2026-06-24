
fn summarize_stage_state(stage_state: &ReportStageState) -> LocalText {
    let stage_keys = [
        (stage_state.overview, "stage_overview"),
        (stage_state.market, "stage_market"),
        (stage_state.fundamentals, "stage_fundamentals"),
        (stage_state.news, "stage_news"),
        (stage_state.sentiment, "stage_sentiment"),
        (stage_state.bull_research, "stage_bull_research"),
        (stage_state.bear_research, "stage_bear_research"),
        (stage_state.research_plan, "stage_research_plan"),
        (stage_state.trader_plan, "stage_trader_plan"),
        (stage_state.risk_debate, "stage_risk_debate"),
        (stage_state.portfolio_decision, "stage_portfolio_decision"),
        (stage_state.reflection, "stage_reflection"),
    ];

    let completed: Vec<serde_json::Value> = stage_keys
        .into_iter()
        .filter(|&(done, _key)| done).map(|(_done, key)| serde_json::Value::String(key.to_string()))
        .collect();

    if completed.is_empty() {
        LocalText::new("stage_state_none")
    } else {
        let mut params = serde_json::Map::new();
        params.insert("stages".to_string(), serde_json::Value::Array(completed));
        LocalText { key: "stage_state_summary".to_string(), params }
    }
}

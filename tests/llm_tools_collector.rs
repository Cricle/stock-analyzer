use stock_analyzer::llm::tools::{AnalysisDataCollector, ScenarioPathData};

#[test]
fn test_collector_full_workflow() {
    let c = AnalysisDataCollector::new();

    // Rating
    c.set_rating("Buy");
    c.set_confidence(75.0);
    c.set_action("accumulate");

    // Prices
    c.set_entry_price(100.0);
    c.set_stop_loss(95.0);
    c.set_target_price(120.0);
    c.set_confirmation_level(105.0);
    c.set_invalidation_level(92.0);

    // Text
    c.set_executive_summary("Buy signal with strong fundamentals");
    c.set_rationale("Technical breakout confirmed by volume");

    // Evidence
    c.add_evidence_point("Revenue growth 20% YoY");
    c.add_key_risk("Market downturn risk");
    c.add_trigger("Close above 105 with volume");

    // Probability
    c.set_probability(0.5, 0.3, 0.2);

    // Scenario
    c.add_scenario_path(ScenarioPathData {
        key: "breakout".to_string(),
        name: "Breakout above resistance".to_string(),
        trigger: "Close above 105".to_string(),
        action: "buy".to_string(),
        ..Default::default()
    });

    let data = c.build();
    assert_eq!(data.rating, "Buy");
    assert_eq!(data.confidence, 75.0);
    assert_eq!(data.entry_price, Some(100.0));
    assert_eq!(data.evidence_points.len(), 1);
    assert_eq!(data.scenario_paths.len(), 1);
}

#[test]
fn test_collector_thread_safety() {
    let collector = AnalysisDataCollector::new();
    let mut handles = vec![];

    for i in 0..10 {
        let c = collector.clone();
        handles.push(std::thread::spawn(move || {
            c.set_confidence(i as f64 * 10.0);
            c.add_evidence_point(format!("evidence {i}"));
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let data = collector.snapshot();
    assert!((0.0..=90.0).contains(&data.confidence));
    assert_eq!(data.evidence_points.len(), 10);
}

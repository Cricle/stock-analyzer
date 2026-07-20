use stock_analyzer::data::MarketDataClient;
use stock_analyzer::llm::LlmClient;
use stock_analyzer::pick::{StockPickQualityTier, run as run_pick_pipeline};
use stock_analyzer::StockPickRequest;

#[tokio::test]
#[ignore]
async fn test_pipeline_with_quality_system() {
    // Setup
    let llm = match LlmClient::from_env() {
        Some(client) => client,
        None => {
            eprintln!("Skipping test: LLM config not available");
            return;
        }
    };
    let market_data = match MarketDataClient::new().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test: MarketDataClient init failed: {}", e);
            return;
        }
    };

    let request = StockPickRequest {
        market: "US".to_string(),
        strategy: Some("test".to_string()),
        analysis_date: Some("2026-07-20".to_string()),
        pick_count: Some(3),
        candidate_limit: Some(10),
        search_depth: Some("light".to_string()),
        history_retrieval: Some(false),
        sector_type: None,
        candidate_symbols: None,
        language: None,
        target_output_mode: None,
    };

    // Execute
    let result = run_pick_pipeline(&market_data, &llm, &request).await;

    // Assert
    assert!(result.is_ok(), "Pipeline should complete successfully");
    let response = result.unwrap();

    println!("\n=== Quality System Integration Test ===");
    println!("Picks generated: {}", response.picks.len());

    // Verify all picks have quality assessment
    for (i, pick) in response.picks.iter().enumerate() {
        println!(
            "\nPick {}: {} ({})",
            i + 1,
            pick.symbol,
            pick.name
        );
        println!("  Final score: {}", pick.objective_assessment.final_score);
        println!("  Quality tier: {:?}", pick.quality_tier);
        println!("  Ready flag: {}", pick.objective_assessment.ready);
        println!("  Gaps: {:?}", pick.objective_assessment.gaps);

        // Verify objective assessment
        assert!(
            pick.objective_assessment.final_score >= 0,
            "Final score should be non-negative"
        );
        assert!(
            pick.objective_assessment.final_score <= 100,
            "Final score should not exceed 100"
        );

        // Verify provenance snapshot exists
        assert!(
            pick.provenance_snapshot.market_data.is_some()
                || pick.provenance_snapshot.fundamentals.is_some(),
            "Provenance snapshot should have at least one data source"
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_quality_tier_classification() {
    let llm = match LlmClient::from_env() {
        Some(client) => client,
        None => {
            eprintln!("Skipping test: LLM config not available");
            return;
        }
    };
    let market_data = match MarketDataClient::new().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test: MarketDataClient init failed: {}", e);
            return;
        }
    };

    let request = StockPickRequest {
        market: "US".to_string(),
        pick_count: Some(3),
        candidate_limit: Some(10),
        strategy: None,
        analysis_date: None,
        search_depth: None,
        history_retrieval: None,
        sector_type: None,
        candidate_symbols: None,
        language: None,
        target_output_mode: None,
    };

    let result = run_pick_pipeline(&market_data, &llm, &request)
        .await
        .expect("Pipeline should complete");

    println!("\n=== Quality Tier Classification Test ===");

    // At least one pick should have a tier assigned
    assert!(!result.picks.is_empty(), "Should generate at least one pick");

    for (i, pick) in result.picks.iter().enumerate() {
        println!(
            "\nPick {}: {} - Score: {}, Tier: {:?}",
            i + 1,
            pick.symbol,
            pick.objective_assessment.final_score,
            pick.quality_tier
        );

        // Verify tier is set to one of the valid values
        assert!(
            matches!(pick.quality_tier, StockPickQualityTier::ProductionReady)
                || matches!(pick.quality_tier, StockPickQualityTier::ReviewRequired)
                || matches!(pick.quality_tier, StockPickQualityTier::DataInsufficient),
            "Quality tier should be one of the valid enum variants"
        );

        // ProductionReady should have high score
        if matches!(pick.quality_tier, StockPickQualityTier::ProductionReady) {
            assert!(
                pick.objective_assessment.final_score >= 80,
                "ProductionReady picks should have score >= 80"
            );
            assert!(
                pick.objective_assessment.ready,
                "ProductionReady picks should have ready flag set"
            );
        }

        // DataInsufficient should have low score
        if matches!(pick.quality_tier, StockPickQualityTier::DataInsufficient) {
            assert!(
                pick.objective_assessment.final_score < 60,
                "DataInsufficient picks should have score < 60"
            );
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_reasoning_consistency_validation() {
    let llm = match LlmClient::from_env() {
        Some(client) => client,
        None => {
            eprintln!("Skipping test: LLM config not available");
            return;
        }
    };
    let market_data = match MarketDataClient::new().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test: MarketDataClient init failed: {}", e);
            return;
        }
    };

    let request = StockPickRequest {
        market: "US".to_string(),
        pick_count: Some(1),
        candidate_limit: Some(5),
        strategy: None,
        analysis_date: None,
        search_depth: None,
        history_retrieval: None,
        sector_type: None,
        candidate_symbols: None,
        language: None,
        target_output_mode: None,
    };

    let result = run_pick_pipeline(&market_data, &llm, &request)
        .await
        .expect("Pipeline should complete");

    println!("\n=== Reasoning Consistency Validation Test ===");

    assert!(!result.picks.is_empty(), "Should generate at least one pick");
    let pick = &result.picks[0];

    println!("Pick: {} ({})", pick.symbol, pick.name);
    println!(
        "Reasoning consistency score: {}",
        pick.objective_assessment.breakdown.reasoning_consistency.score
    );
    println!(
        "Reasoning consistency rationale: {}",
        pick.objective_assessment
            .breakdown
            .reasoning_consistency
            .rationale
            .as_str()
    );

    // Reasoning consistency dimension should be scored
    assert!(
        pick.objective_assessment.breakdown.reasoning_consistency.score >= 0,
        "Reasoning consistency score should be non-negative"
    );
    assert!(
        pick.objective_assessment.breakdown.reasoning_consistency.score <= 20,
        "Reasoning consistency score should not exceed 20 (max for this dimension)"
    );

    // Rationale should be populated
    assert!(
        !pick
            .objective_assessment
            .breakdown
            .reasoning_consistency
            .rationale
            .as_str()
            .is_empty(),
        "Reasoning consistency rationale should be populated"
    );
}

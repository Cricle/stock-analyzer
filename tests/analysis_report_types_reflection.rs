use stock_analyzer::analysis::{
    ActionScenarioPath, CatalystScoreCard, CatalystScoreItem, LocalText, MissingEvidenceLadder,
    Rating, ReviewChecklist, ReviewItem, StructuredPortfolioDecision, StructuredResearchPlan,
    StructuredTraderPlan,
};

#[test]
fn action_scenario_path_serde_roundtrip() {
    let p = ActionScenarioPath {
        key: "bull".into(),
        name: LocalText::new("bull_case"),
        trigger: LocalText::new("breakout"),
        action: LocalText::new("add"),
        risk_boundary: LocalText::new("stop"),
        position_sizing: LocalText::new("50%"),
        stop_level: LocalText::new("145"),
        sizing_blocked: false,
    };
    let json = serde_json::to_string(&p).unwrap();
    let restored: ActionScenarioPath = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.key, "bull");
    assert!(!restored.sizing_blocked);
}

#[test]
fn structured_research_plan_serde_roundtrip() {
    let p = StructuredResearchPlan {
        recommendation: LocalText::new("buy"),
        confidence: LocalText::new("high"),
        risk_assessment: LocalText::new("moderate"),
        rationale: LocalText::new("strong"),
        strategic_actions: LocalText::new("accumulate"),
        missing_evidence_ladder: MissingEvidenceLadder::default(),
        trigger_checklist: vec!["earnings".into()],
        accounting_scope_hypothesis: "consolidated".into(),
        markdown: "md".into(),
    };
    let json = serde_json::to_string(&p).unwrap();
    let restored: StructuredResearchPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.recommendation.key, "buy");
}

#[test]
fn structured_trader_plan_serde_roundtrip() {
    let p = StructuredTraderPlan {
        action: LocalText::new("buy"),
        raw_action: "Buy".into(),
        calibrated_action: "Buy".into(),
        reasoning: LocalText::new("good"),
        entry_price: "150".into(),
        stop_loss: "145".into(),
        confirmation_level: "152".into(),
        target_reference: "160".into(),
        target_condition: "".into(),
        time_horizon: "2w".into(),
        position_sizing: "30%".into(),
        proposal: LocalText::new("buy_dip"),
        execution_trigger_checklist: vec!["vol".into()],
        blocking_gaps: vec![],
        time_stop_deadline: "2025-02-01".into(),
        time_stop_reason: "earnings".into(),
        markdown: "md".into(),
    };
    let json = serde_json::to_string(&p).unwrap();
    let restored: StructuredTraderPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.entry_price, "150");
}

#[test]
fn structured_portfolio_decision_serde_roundtrip() {
    let d = StructuredPortfolioDecision {
        rating: Rating::Buy,
        raw_rating: "Buy".into(),
        calibrated_rating: "Buy".into(),
        confidence: LocalText::new("high"),
        risk_assessment: LocalText::new("mod"),
        executive_summary: LocalText::new("sum"),
        investment_thesis: LocalText::new("thesis"),
        rationale: LocalText::new("rat"),
        price_target: "160".into(),
        confirmation_level: "152".into(),
        invalidation_level: "145".into(),
        target_type: "point".into(),
        target_reference: "160".into(),
        target_condition: "".into(),
        time_horizon: "1m".into(),
        missing_evidence_ladder: MissingEvidenceLadder::default(),
        trigger_checklist: vec!["e".into()],
        markdown: "md".into(),
    };
    let json = serde_json::to_string(&d).unwrap();
    let restored: StructuredPortfolioDecision = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.rating, Rating::Buy);
}

#[test]
fn missing_evidence_ladder_serde_roundtrip() {
    let l = MissingEvidenceLadder {
        tolerable_gaps: vec!["g1".into()],
        manageable_gaps: vec!["g2".into()],
        blocking_gaps: vec!["g3".into()],
    };
    let json = serde_json::to_string(&l).unwrap();
    let restored: MissingEvidenceLadder = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.blocking_gaps.len(), 1);
}

#[test]
fn catalyst_score_card_serde_roundtrip() {
    let c = CatalystScoreCard {
        event_name: "Q2".into(),
        items: vec![CatalystScoreItem {
            question: LocalText::new("q"),
            score: 1,
            evidence: LocalText::new("e"),
        }],
        total_score: 8,
        max_score: 10,
        interpretation: LocalText::new("pos"),
        recommended_action: LocalText::new("act"),
    };
    let json = serde_json::to_string(&c).unwrap();
    let restored: CatalystScoreCard = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.total_score, 8);
}

#[test]
fn review_checklist_serde_roundtrip() {
    let c = ReviewChecklist {
        daily: vec![ReviewItem {
            check: LocalText::new("price"),
            category: "price".into(),
            priority: "high".into(),
        }],
        weekly: vec![ReviewItem {
            check: LocalText::new("earnings"),
            category: "fund".into(),
            priority: "med".into(),
        }],
    };
    let json = serde_json::to_string(&c).unwrap();
    let restored: ReviewChecklist = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.daily.len(), 1);
    assert_eq!(restored.weekly.len(), 1);
}

#[test]
fn all_plan_defaults() {
    assert!(ActionScenarioPath::default().key.is_empty());
    assert!(StructuredResearchPlan::default().recommendation.is_empty());
    assert!(StructuredTraderPlan::default().entry_price.is_empty());
    assert_eq!(StructuredPortfolioDecision::default().rating, Rating::Hold);
    assert!(MissingEvidenceLadder::default().blocking_gaps.is_empty());
    assert!(CatalystScoreCard::default().event_name.is_empty());
    assert!(ReviewChecklist::default().daily.is_empty());
    assert!(ReviewItem::default().category.is_empty());
}

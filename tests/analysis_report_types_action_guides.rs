use stock_analyzer::analysis::{
    ActionBreakdown, ActionScenarioPath, AudienceActionGuide, DirectionBreakdown, LocalText,
    ReportActionGuides, ReportSection, ScoreDimension, SignedScoreDimension,
};

#[test]
fn direction_breakdown_serde_roundtrip() {
    let d = DirectionBreakdown {
        market: SignedScoreDimension {
            score: 15,
            min_score: -25,
            max_score: 25,
            rationale: LocalText::default(),
        },
        fundamentals: SignedScoreDimension {
            score: 10,
            min_score: -25,
            max_score: 25,
            rationale: LocalText::default(),
        },
        news: SignedScoreDimension {
            score: 5,
            min_score: -25,
            max_score: 25,
            rationale: LocalText::default(),
        },
        sentiment: SignedScoreDimension {
            score: 3,
            min_score: -25,
            max_score: 25,
            rationale: LocalText::default(),
        },
        risk_adjustment: SignedScoreDimension {
            score: -2,
            min_score: -15,
            max_score: 15,
            rationale: LocalText::default(),
        },
        total_score: 31,
        implied_rating: LocalText::new("Overweight"),
    };
    let json = serde_json::to_string(&d).unwrap();
    let restored: DirectionBreakdown = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.total_score, 31);
}

#[test]
fn signed_score_dimension_serde_roundtrip() {
    let d = SignedScoreDimension {
        score: 15,
        min_score: -25,
        max_score: 25,
        rationale: LocalText::new("bullish"),
    };
    let json = serde_json::to_string(&d).unwrap();
    let restored: SignedScoreDimension = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.score, 15);
    assert_eq!(restored.min_score, -25);
}

#[test]
fn action_breakdown_serde_roundtrip() {
    let b = ActionBreakdown {
        alignment: ScoreDimension {
            score: 14,
            max_score: 20,
            rationale: LocalText::default(),
        },
        execution_levels: ScoreDimension {
            score: 10,
            max_score: 15,
            rationale: LocalText::default(),
        },
        sizing_discipline: ScoreDimension {
            score: 8,
            max_score: 15,
            rationale: LocalText::default(),
        },
        horizon_clarity: ScoreDimension {
            score: 7,
            max_score: 10,
            rationale: LocalText::default(),
        },
        reward_to_risk: ScoreDimension {
            score: 12,
            max_score: 15,
            rationale: LocalText::default(),
        },
        total_score: 51,
    };
    let json = serde_json::to_string(&b).unwrap();
    let restored: ActionBreakdown = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.total_score, 51);
}

#[test]
fn report_section_serde_roundtrip() {
    let s = ReportSection {
        key: "summary".into(),
        title: "Executive Summary".into(),
        content: "Buy recommendation".into(),
    };
    let json = serde_json::to_string(&s).unwrap();
    let restored: ReportSection = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.key, "summary");
}

#[test]
fn report_action_guides_serde_roundtrip() {
    let g = ReportActionGuides {
        holders: AudienceActionGuide {
            audience: LocalText::new("holders"),
            priority: "high".into(),
            ..Default::default()
        },
        buyers: AudienceActionGuide::default(),
        watchers: AudienceActionGuide::default(),
    };
    let json = serde_json::to_string(&g).unwrap();
    let restored: ReportActionGuides = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.holders.priority, "high");
}

#[test]
fn audience_action_guide_serde_roundtrip() {
    let g = AudienceActionGuide {
        audience: LocalText::new("buyers"),
        user_state: LocalText::new("no_position"),
        priority: "medium".into(),
        stance: LocalText::new("buy_on_dip"),
        summary: LocalText::new("summary"),
        principle: LocalText::new("principle"),
        entry_reference: "150".into(),
        invalidation_reference: "145".into(),
        target_reference: "160".into(),
        confirmation_reference: "152".into(),
        time_horizon: "2w".into(),
        sizing_reference: LocalText::new("30%"),
        actions: vec![LocalText::new("buy")],
        avoid: vec![LocalText::new("chase")],
        review_points: vec![LocalText::new("check_volume")],
        scenario_paths: vec![ActionScenarioPath::default()],
    };
    let json = serde_json::to_string(&g).unwrap();
    let restored: AudienceActionGuide = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.priority, "medium");
    assert_eq!(restored.actions.len(), 1);
    assert_eq!(restored.scenario_paths.len(), 1);
}

#[test]
fn all_defaults() {
    let d = DirectionBreakdown::default();
    assert_eq!(d.total_score, 0);

    let b = ActionBreakdown::default();
    assert_eq!(b.total_score, 0);

    let s = ReportSection::default();
    assert!(s.key.is_empty());

    let g = ReportActionGuides::default();
    assert!(g.holders.audience.is_empty());

    let a = AudienceActionGuide::default();
    assert!(a.actions.is_empty());
}

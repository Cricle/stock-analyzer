use stock_analyzer::analysis::{
    CalibrationBias, CalibrationSummary, HistoricalCalibrationStats, LocalText, ScoreDimension,
    SetupMatchExplanation, TradeSetupQuality,
};

#[test]
fn trade_setup_quality_serde_roundtrip() {
    let t = TradeSetupQuality {
        score: 8,
        max_score: 10,
        ready: true,
        label: LocalText::new("strong_setup"),
        rationale: LocalText::new("good_levels"),
        strengths: vec![LocalText::new("clear_entry")],
        gaps: vec![LocalText::new("missing_volume")],
    };
    let json = serde_json::to_string(&t).unwrap();
    let restored: TradeSetupQuality = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.score, 8);
    assert!(restored.ready);
}

#[test]
fn calibration_summary_serde_roundtrip() {
    let c = CalibrationSummary {
        threshold_tightened: true,
        memory_threshold_tightened: false,
        min_confidence_score: 60,
        min_action_score: 50,
        direction_floor_abs: 15,
        strong_direction_abs: 65,
        direction_threshold_penalty: 5,
        historical: HistoricalCalibrationStats {
            sample_count: 10,
            hit_rate: 0.7,
            avg_alpha_return: 0.05,
        },
        setup_calibration_sample_count: 8,
        setup_match_count: 5,
        setup_pending_match_count: 2,
        setup_match_explanation: SetupMatchExplanation {
            reason_code: "exact".into(),
            summary: "matched".into(),
            details: vec!["d1".into()],
            fallback_used: false,
            fallback_sample_count: 0,
        },
        setup_match_quality: ScoreDimension {
            score: 8,
            max_score: 10,
            rationale: LocalText::default(),
        },
        setup_direction_alignment: ScoreDimension {
            score: 7,
            max_score: 10,
            rationale: LocalText::default(),
        },
        calibration_bias: CalibrationBias {
            direction: LocalText::new("bullish"),
            magnitude: LocalText::new("moderate"),
            rationale: LocalText::new("bias_reason"),
        },
        decision_narrative: LocalText::new("narrative"),
    };
    let json = serde_json::to_string(&c).unwrap();
    let restored: CalibrationSummary = serde_json::from_str(&json).unwrap();
    assert!(restored.threshold_tightened);
    assert_eq!(restored.min_confidence_score, 60);
}

#[test]
fn setup_match_explanation_serde_roundtrip() {
    let e = SetupMatchExplanation {
        reason_code: "fuzzy".into(),
        summary: "close match".into(),
        details: vec!["d1".into(), "d2".into()],
        fallback_used: true,
        fallback_sample_count: 3,
    };
    let json = serde_json::to_string(&e).unwrap();
    let restored: SetupMatchExplanation = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.reason_code, "fuzzy");
    assert!(restored.fallback_used);
}

#[test]
fn all_defaults() {
    let t = TradeSetupQuality::default();
    assert_eq!(t.score, 0);
    assert!(!t.ready);
    assert!(t.strengths.is_empty());

    let c = CalibrationSummary::default();
    assert!(!c.threshold_tightened);
    assert_eq!(c.min_confidence_score, 0);

    let e = SetupMatchExplanation::default();
    assert!(e.reason_code.is_empty());
    assert!(!e.fallback_used);
}

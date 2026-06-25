use sa::analysis::{
    CalibrationBias, ConfidenceBreakdown, ConfidenceCap, HistoricalCalibrationStats, LocalText,
    ReferenceFactItem, ReportDiagnosticItem, ReportDiagnostics, ReportReferenceSnapshot,
    ResearchReliability, ScoreDimension,
};

#[test]
fn calibration_bias_serde_roundtrip() {
    let b = CalibrationBias {
        direction: LocalText::new("bullish"),
        magnitude: LocalText::new("moderate"),
        rationale: LocalText::new("reason"),
    };
    let json = serde_json::to_string(&b).unwrap();
    let restored: CalibrationBias = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.direction.key, "bullish");
}

#[test]
fn report_diagnostics_serde_roundtrip() {
    let d = ReportDiagnostics {
        market: vec![ReportDiagnosticItem {
            code: "m1".into(),
            severity: "warning".into(),
            message: LocalText::new("missing_data"),
            details: vec!["d1".into()],
            related_blocking_gaps: vec!["g1".into()],
            related_trigger_checklist: vec!["t1".into()],
            elevated_to_execution_blocking_gap: false,
        }],
        fundamentals: vec![],
        news: vec![],
        availability: vec![],
    };
    let json = serde_json::to_string(&d).unwrap();
    let restored: ReportDiagnostics = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.market.len(), 1);
    assert_eq!(restored.market[0].code, "m1");
}

#[test]
fn report_diagnostic_item_serde_roundtrip() {
    let i = ReportDiagnosticItem {
        code: "fundamentals_period_mixed".into(),
        severity: "error".into(),
        message: LocalText::new("period_mixed"),
        details: vec!["Q1 and Q2 mixed".into()],
        related_blocking_gaps: vec!["gap1".into()],
        related_trigger_checklist: vec!["check1".into()],
        elevated_to_execution_blocking_gap: true,
    };
    let json = serde_json::to_string(&i).unwrap();
    let restored: ReportDiagnosticItem = serde_json::from_str(&json).unwrap();
    assert!(restored.elevated_to_execution_blocking_gap);
}

#[test]
fn report_reference_snapshot_serde_roundtrip() {
    let r = ReportReferenceSnapshot {
        market: vec![ReferenceFactItem {
            key: "pe_ratio".into(),
            label: "P/E".into(),
            value: "25.3".into(),
            emphasis: "high".into(),
            url: "http://test".into(),
            summary: "high PE".into(),
        }],
        fundamentals: vec![],
        news: vec![],
        memory: vec![],
    };
    let json = serde_json::to_string(&r).unwrap();
    let restored: ReportReferenceSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.market.len(), 1);
}

#[test]
fn reference_fact_item_serde_roundtrip() {
    let i = ReferenceFactItem {
        key: "revenue".into(),
        label: "Revenue".into(),
        value: "100B".into(),
        emphasis: "strong".into(),
        url: "http://test".into(),
        summary: "record revenue".into(),
    };
    let json = serde_json::to_string(&i).unwrap();
    let restored: ReferenceFactItem = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.key, "revenue");
}

#[test]
fn historical_calibration_stats_serde_roundtrip() {
    let s = HistoricalCalibrationStats {
        sample_count: 15,
        hit_rate: 0.73,
        avg_alpha_return: 0.045,
    };
    let json = serde_json::to_string(&s).unwrap();
    let restored: HistoricalCalibrationStats = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.sample_count, 15);
    assert!((restored.hit_rate - 0.73).abs() < 0.001);
}

#[test]
fn confidence_breakdown_serde_roundtrip() {
    let b = ConfidenceBreakdown {
        data_quality: ScoreDimension {
            score: 18,
            max_score: 20,
            rationale: LocalText::default(),
        },
        trend_confirmation: ScoreDimension {
            score: 15,
            max_score: 20,
            rationale: LocalText::default(),
        },
        fundamental_confirmation: ScoreDimension {
            score: 14,
            max_score: 20,
            rationale: LocalText::default(),
        },
        catalyst_quality: ScoreDimension {
            score: 10,
            max_score: 15,
            rationale: LocalText::default(),
        },
        historical_transferability: ScoreDimension {
            score: 7,
            max_score: 10,
            rationale: LocalText::default(),
        },
        cross_agent_consistency: ScoreDimension {
            score: 12,
            max_score: 15,
            rationale: LocalText::default(),
        },
        risk_clarity: ScoreDimension {
            score: 8,
            max_score: 10,
            rationale: LocalText::default(),
        },
        total_before_caps: 84,
        final_score: 80,
        applied_cap: 85,
    };
    let json = serde_json::to_string(&b).unwrap();
    let restored: ConfidenceBreakdown = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.total_before_caps, 84);
    assert_eq!(restored.final_score, 80);
}

#[test]
fn score_dimension_serde_roundtrip() {
    let d = ScoreDimension {
        score: 15,
        max_score: 20,
        rationale: LocalText::new("good"),
    };
    let json = serde_json::to_string(&d).unwrap();
    let restored: ScoreDimension = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.score, 15);
}

#[test]
fn confidence_cap_serde_roundtrip() {
    let c = ConfidenceCap {
        key: "missing_core_data".into(),
        label: LocalText::new("Missing Core Data"),
        cap: 80,
        reason: LocalText::new("only 2 of 4 core reports present"),
    };
    let json = serde_json::to_string(&c).unwrap();
    let restored: ConfidenceCap = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.cap, 80);
}

#[test]
fn research_reliability_serde_roundtrip() {
    let r = ResearchReliability {
        score: 75,
        max_score: 100,
        label: LocalText::new("good"),
        rationale: LocalText::new("solid data"),
        strengths: vec![LocalText::new("s1")],
        constraints: vec![LocalText::new("c1")],
    };
    let json = serde_json::to_string(&r).unwrap();
    let restored: ResearchReliability = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.score, 75);
}

#[test]
fn all_defaults() {
    assert!(CalibrationBias::default().direction.is_empty());
    assert!(ReportDiagnostics::default().market.is_empty());
    assert!(ReportDiagnosticItem::default().code.is_empty());
    assert!(ReportReferenceSnapshot::default().market.is_empty());
    assert!(ReferenceFactItem::default().key.is_empty());
    assert_eq!(HistoricalCalibrationStats::default().sample_count, 0);
    assert_eq!(ConfidenceBreakdown::default().final_score, 0);
    assert_eq!(ScoreDimension::default().score, 0);
    assert!(ConfidenceCap::default().key.is_empty());
    assert_eq!(ResearchReliability::default().score, 0);
}

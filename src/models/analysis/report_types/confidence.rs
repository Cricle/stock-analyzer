
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CalibrationBias {
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub direction: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub magnitude: LocalText,
    #[serde(default, skip_serializing)]
    pub rationale: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportDiagnostics {
    #[serde(default)]
    pub market: Vec<ReportDiagnosticItem>,
    #[serde(default)]
    pub fundamentals: Vec<ReportDiagnosticItem>,
    #[serde(default)]
    pub news: Vec<ReportDiagnosticItem>,
    #[serde(default)]
    pub availability: Vec<ReportDiagnosticItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportDiagnosticItem {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub message: LocalText,
    #[serde(default)]
    pub details: Vec<String>,
    #[serde(default)]
    pub related_blocking_gaps: Vec<String>,
    #[serde(default)]
    pub related_trigger_checklist: Vec<String>,
    #[serde(default)]
    pub elevated_to_execution_blocking_gap: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportReferenceSnapshot {
    #[serde(default)]
    pub market: Vec<ReferenceFactItem>,
    #[serde(default)]
    pub fundamentals: Vec<ReferenceFactItem>,
    #[serde(default)]
    pub news: Vec<ReferenceFactItem>,
    #[serde(default)]
    pub memory: Vec<ReferenceFactItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReferenceFactItem {
    #[serde(default)]
    pub key: String,
    #[serde(default, skip_serializing)]
    pub label: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub emphasis: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub summary: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HistoricalCalibrationStats {
    #[serde(default)]
    pub sample_count: usize,
    #[serde(default)]
    pub hit_rate: f64,
    #[serde(default)]
    pub avg_alpha_return: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfidenceBreakdown {
    #[serde(default)]
    pub data_quality: ScoreDimension,
    #[serde(default)]
    pub trend_confirmation: ScoreDimension,
    #[serde(default)]
    pub fundamental_confirmation: ScoreDimension,
    #[serde(default)]
    pub catalyst_quality: ScoreDimension,
    #[serde(default)]
    pub historical_transferability: ScoreDimension,
    #[serde(default)]
    pub cross_agent_consistency: ScoreDimension,
    #[serde(default)]
    pub risk_clarity: ScoreDimension,
    #[serde(default)]
    pub total_before_caps: i32,
    #[serde(default)]
    pub final_score: i32,
    #[serde(default)]
    pub applied_cap: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScoreDimension {
    pub score: i32,
    pub max_score: i32,
    #[serde(default, skip_serializing)]
    pub rationale: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConfidenceCap {
    pub key: String,
    #[serde(default, skip_serializing)]
    pub label: LocalText,
    pub cap: i32,
    #[serde(default, skip_serializing)]
    pub reason: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResearchReliability {
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub max_score: i32,
    #[serde(default)]
    pub label: LocalText,
    #[serde(default, skip_serializing)]
    pub rationale: LocalText,
    #[serde(default)]
    pub strengths: Vec<LocalText>,
    #[serde(default)]
    pub constraints: Vec<LocalText>,
}

#[cfg(test)]
mod confidence_tests {
    use super::super::*;

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
            data_quality: ScoreDimension { score: 18, max_score: 20, rationale: LocalText::default() },
            trend_confirmation: ScoreDimension { score: 15, max_score: 20, rationale: LocalText::default() },
            fundamental_confirmation: ScoreDimension { score: 14, max_score: 20, rationale: LocalText::default() },
            catalyst_quality: ScoreDimension { score: 10, max_score: 15, rationale: LocalText::default() },
            historical_transferability: ScoreDimension { score: 7, max_score: 10, rationale: LocalText::default() },
            cross_agent_consistency: ScoreDimension { score: 12, max_score: 15, rationale: LocalText::default() },
            risk_clarity: ScoreDimension { score: 8, max_score: 10, rationale: LocalText::default() },
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
        let d = ScoreDimension { score: 15, max_score: 20, rationale: LocalText::new("good") };
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
}

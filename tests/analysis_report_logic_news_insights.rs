use stock_analyzer::analysis::{
    DecisionAction, DecisionView, ReferenceFactItem, ReportDiagnosticItem, ReportReferenceSnapshot,
};
use stock_analyzer::analysis::{derive_evidence_cards, has_report_diagnostic, news_watch_next_summary};

#[test]
fn derive_evidence_cards_empty_references() {
    let refs = ReportReferenceSnapshot::default();
    let cards = derive_evidence_cards(&refs);
    assert!(cards.is_empty());
}

#[test]
fn derive_evidence_cards_market_items() {
    let refs = ReportReferenceSnapshot {
        market: vec![ReferenceFactItem {
            key: "pe".into(),
            value: "25.3".into(),
            emphasis: "primary".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let cards = derive_evidence_cards(&refs);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].category, "market");
    assert_eq!(cards[0].direction, "primary");
}

#[test]
fn derive_evidence_cards_direction_mapping() {
    let refs = ReportReferenceSnapshot {
        market: vec![
            ReferenceFactItem {
                key: "k1".into(),
                value: "v1".into(),
                emphasis: "success".into(),
                ..Default::default()
            },
            ReferenceFactItem {
                key: "k2".into(),
                value: "v2".into(),
                emphasis: "warning".into(),
                ..Default::default()
            },
            ReferenceFactItem {
                key: "k3".into(),
                value: "v3".into(),
                emphasis: "other".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let cards = derive_evidence_cards(&refs);
    assert_eq!(cards[0].direction, "positive");
    assert_eq!(cards[1].direction, "caution");
    assert_eq!(cards[2].direction, "neutral");
}

#[test]
fn derive_evidence_cards_max_8_per_category() {
    let items: Vec<ReferenceFactItem> = (0..10)
        .map(|i| ReferenceFactItem {
            key: format!("k{}", i),
            value: format!("v{}", i),
            ..Default::default()
        })
        .collect();
    let refs = ReportReferenceSnapshot {
        market: items,
        ..Default::default()
    };
    let cards = derive_evidence_cards(&refs);
    assert_eq!(cards.len(), 8);
}

#[test]
fn derive_evidence_cards_claim_from_value() {
    let refs = ReportReferenceSnapshot {
        market: vec![ReferenceFactItem {
            key: "pe".into(),
            value: "25.3".into(),
            summary: "".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let cards = derive_evidence_cards(&refs);
    assert_eq!(cards[0].claim.key, "25.3");
}

#[test]
fn derive_evidence_cards_claim_from_summary() {
    let refs = ReportReferenceSnapshot {
        market: vec![ReferenceFactItem {
            key: "pe".into(),
            value: "".into(),
            summary: "high PE".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let cards = derive_evidence_cards(&refs);
    assert_eq!(cards[0].claim.key, "high PE");
}

#[test]
fn news_watch_next_summary_early_probe() {
    let mut decision = DecisionView::default();
    decision.early_probe_allowed = true;
    let result = news_watch_next_summary(&decision);
    assert_eq!(result.key, "watch_price_volume_follow_through");
}

#[test]
fn news_watch_next_summary_wait_retest() {
    let mut decision = DecisionView::default();
    decision.action = DecisionAction::WaitRetest;
    let result = news_watch_next_summary(&decision);
    assert_eq!(result.key, "watch_retest_acceptance");
}

#[test]
fn news_watch_next_summary_default() {
    let decision = DecisionView::default();
    let result = news_watch_next_summary(&decision);
    assert_eq!(result.key, "watch_confirmation_breakout");
}

#[test]
fn has_report_diagnostic_found() {
    let items = vec![ReportDiagnosticItem {
        code: "market_data_unavailable".into(),
        ..Default::default()
    }];
    assert!(has_report_diagnostic(&items, "market_data_unavailable"));
}

#[test]
fn has_report_diagnostic_not_found() {
    let items = vec![ReportDiagnosticItem {
        code: "other".into(),
        ..Default::default()
    }];
    assert!(!has_report_diagnostic(&items, "market_data_unavailable"));
}

#[test]
fn has_report_diagnostic_empty() {
    assert!(!has_report_diagnostic(&[], "any"));
}

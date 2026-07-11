use stock_analyzer::analysis::{
    IcDisciplineView, IcNavigatorView, LocalText, NewsInsight, PriceContext, ProbabilityDriver,
    ProbabilityView, ProfitRiskView, ReportEvidenceCard, RiskControl, TechnicalIndicatorCategory,
    TechnicalIndicatorConclusion, TechnicalIndicatorItem, TechnicalIndicatorView,
};

#[test]
fn price_context_serde_roundtrip() {
    let p = PriceContext {
        current_price: Some(150.0),
        lookback_days: 30,
        high_price: Some(155.0),
        high_date: "2025-01-10".into(),
        low_price: Some(145.0),
        low_date: "2025-01-05".into(),
        distance_to_high_pct: Some(-3.2),
        distance_to_low_pct: Some(3.4),
        range_pct: Some(6.9),
        latest_volume: Some(1000000),
        volume_change_pct: Some(10.5),
    };
    let json = serde_json::to_string(&p).unwrap();
    let restored: PriceContext = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.current_price, Some(150.0));
    assert_eq!(restored.lookback_days, 30);
}

#[test]
fn probability_view_serde_roundtrip() {
    let p = ProbabilityView {
        upside_probability_pct: 60.0,
        upside_target: Some(160.0),
        upside_pct: Some(6.7),
        downside_probability_pct: 25.0,
        downside_target: Some(140.0),
        downside_pct: Some(-6.7),
        sideways_probability_pct: 15.0,
        risk_probability_pct: 25.0,
        confidence_band: LocalText::new("medium"),
        drivers: vec![ProbabilityDriver {
            key: "earnings".into(),
            direction: "up".into(),
            value: "high".into(),
            evidence_keys: vec!["e1".into()],
        }],
        profit_target: Some(160.0),
        stop_loss: Some(140.0),
    };
    let json = serde_json::to_string(&p).unwrap();
    let restored: ProbabilityView = serde_json::from_str(&json).unwrap();
    assert!((restored.upside_probability_pct - 60.0).abs() < 0.01);
    assert_eq!(restored.drivers.len(), 1);
}

#[test]
fn profit_risk_view_serde_roundtrip() {
    let p = ProfitRiskView {
        upside_pct: Some(10.0),
        downside_pct: Some(-5.0),
        reward_risk_ratio: Some(2.0),
        current_position_reward_risk_ratio: Some(1.5),
        max_loss_reference: Some(140.0),
        risk_budget: LocalText::new("5%"),
        actionability: LocalText::new("high"),
        calc_entry: Some(150.0),
        calc_target: Some(165.0),
        calc_stop: Some(140.0),
        trade_direction: "long".into(),
        trade_summary: "Buy at 150, target 165, stop 140".into(),
    };
    let json = serde_json::to_string(&p).unwrap();
    let restored: ProfitRiskView = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.reward_risk_ratio, Some(2.0));
}

#[test]
fn ic_navigator_view_serde_roundtrip() {
    let v = IcNavigatorView {
        verdict: LocalText::new("proceed"),
        primary_path_key: "bull".into(),
        path_probability_pct: 65.0,
        confidence_band: "medium".into(),
        can_act_now: true,
        early_probe_allowed: false,
        upgrade_condition: LocalText::new("breakout"),
        abort_condition: LocalText::new("breakdown"),
        responsibility: LocalText::new("manage risk"),
    };
    let json = serde_json::to_string(&v).unwrap();
    let restored: IcNavigatorView = serde_json::from_str(&json).unwrap();
    assert!(restored.can_act_now);
}

#[test]
fn ic_discipline_view_serde_roundtrip() {
    let v = IcDisciplineView {
        state: LocalText::new("ready"),
        reason_codes: vec!["rc1".into()],
        next_action_code: LocalText::new("buy"),
        reward_risk_ratio: Some(2.0),
        current_position_reward_risk_ratio: None,
        rsi: Some(65.0),
        macd: Some(0.5),
        upside_probability_pct: 60.0,
        downside_probability_pct: 25.0,
        risk_probability_pct: 15.0,
        current_price: Some(150.0),
        confirmation_price: Some(152.0),
        invalidation_price: Some(145.0),
        upside_pct: Some(6.7),
        downside_pct: Some(-3.3),
        technical_signal_codes: vec!["bullish".into()],
        signal_resolution: Default::default(),
    };
    let json = serde_json::to_string(&v).unwrap();
    let restored: IcDisciplineView = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.rsi, Some(65.0));
}

#[test]
fn technical_indicator_view_serde_roundtrip() {
    let v = TechnicalIndicatorView {
        categories: vec![TechnicalIndicatorCategory {
            key: "momentum".into(),
            display_mode: "both".into(),
            signal_attribute: "bullish".into(),
            indicators: vec![TechnicalIndicatorItem {
                key: "rsi".into(),
                value: Some(65.0),
                signal_code: "bullish".into(),
                interpretation_code: "overbought".into(),
                display_mode: "both".into(),
                signal_attribute: "bullish".into(),
            }],
        }],
        conclusions: vec![TechnicalIndicatorConclusion {
            key: "momentum_bullish".into(),
            severity: "info".into(),
            evidence_keys: vec!["rsi".into()],
        }],
    };
    let json = serde_json::to_string(&v).unwrap();
    let restored: TechnicalIndicatorView = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.categories.len(), 1);
    assert_eq!(restored.conclusions.len(), 1);
}

#[test]
fn report_evidence_card_serde_roundtrip() {
    let c = ReportEvidenceCard {
        key: "revenue".into(),
        category: "fundamental".into(),
        value: "100B".into(),
        unit: "USD".into(),
        direction: "up".into(),
        strength: "strong".into(),
        source: "10-K".into(),
        claim: LocalText::new("revenue_growth"),
    };
    let json = serde_json::to_string(&c).unwrap();
    let restored: ReportEvidenceCard = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.key, "revenue");
}

#[test]
fn news_insight_serde_roundtrip() {
    let n = NewsInsight {
        title: "Apple Earnings".into(),
        published_at: "2025-01-15".into(),
        source: "Reuters".into(),
        url: "http://test.com".into(),
        fact_summary: LocalText::new("record_revenue"),
        interpretation: LocalText::new("bullish"),
        impact_direction: LocalText::new("positive"),
        impact_strength: LocalText::new("strong"),
        what_it_confirms: LocalText::new("growth"),
        what_to_watch_next: LocalText::new("guidance"),
        published_before_analysis: true,
    };
    let json = serde_json::to_string(&n).unwrap();
    let restored: NewsInsight = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.title, "Apple Earnings");
    assert!(restored.published_before_analysis);
}

#[test]
fn risk_control_serde_roundtrip() {
    let r = RiskControl {
        risk_name: LocalText::new("competition"),
        probability_pct: 30.0,
        impact: LocalText::new("high"),
        trigger: LocalText::new("market_share_decline"),
        defense_action: LocalText::new("reduce_position"),
        invalidation_level: "140".into(),
        monitoring_signal: LocalText::new("quarterly_earnings"),
    };
    let json = serde_json::to_string(&r).unwrap();
    let restored: RiskControl = serde_json::from_str(&json).unwrap();
    assert!((restored.probability_pct - 30.0).abs() < 0.01);
}

#[test]
fn probability_driver_serde_roundtrip() {
    let d = ProbabilityDriver {
        key: "earnings".into(),
        direction: "up".into(),
        value: "beat".into(),
        evidence_keys: vec!["e1".into(), "e2".into()],
    };
    let json = serde_json::to_string(&d).unwrap();
    let restored: ProbabilityDriver = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.evidence_keys.len(), 2);
}

#[test]
fn all_defaults() {
    assert!(PriceContext::default().current_price.is_none());
    assert!(ProbabilityView::default().drivers.is_empty());
    assert!(ProfitRiskView::default().upside_pct.is_none());
    assert!(IcNavigatorView::default().primary_path_key.is_empty());
    assert!(TechnicalIndicatorView::default().categories.is_empty());
    assert!(ReportEvidenceCard::default().key.is_empty());
    assert!(NewsInsight::default().title.is_empty());
    assert!(RiskControl::default().probability_pct == 0.0);
}

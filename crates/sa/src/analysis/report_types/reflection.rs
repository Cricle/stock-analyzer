
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActionScenarioPath {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: LocalText,
    #[serde(default)]
    pub trigger: LocalText,
    #[serde(default)]
    pub action: LocalText,
    #[serde(default)]
    pub risk_boundary: LocalText,
    /// Per-path position sizing guidance, e.g. "计划仓位的50%（不超过总资金5%）"
    #[serde(default)]
    pub position_sizing: LocalText,
    /// Per-path specific stop-loss or invalidation price level
    #[serde(default)]
    pub stop_level: LocalText,
    /// When true, `position_sizing` is intentionally empty because IC
    /// discipline forbids new positions (e.g. "no_attack" state).  The
    /// frontend should render this as "observation only" without inventing
    /// a sizing number.
    #[serde(default)]
    pub sizing_blocked: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StructuredResearchPlan {
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub recommendation: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub confidence: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub risk_assessment: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub rationale: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub strategic_actions: LocalText,
    #[serde(default)]
    pub missing_evidence_ladder: MissingEvidenceLadder,
    #[serde(default)]
    pub trigger_checklist: Vec<String>,
    #[serde(default)]
    pub accounting_scope_hypothesis: String,
    #[serde(default, skip_serializing)]
    pub markdown: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StructuredTraderPlan {
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub action: LocalText,
    #[serde(default)]
    pub raw_action: String,
    #[serde(default)]
    pub calibrated_action: String,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub reasoning: LocalText,
    pub entry_price: String,
    pub stop_loss: String,
    #[serde(default)]
    pub confirmation_level: String,
    #[serde(default)]
    pub target_reference: String,
    #[serde(default)]
    pub target_condition: String,
    #[serde(default)]
    pub time_horizon: String,
    pub position_sizing: String,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub proposal: LocalText,
        #[serde(default)]
    pub execution_trigger_checklist: Vec<String>,
    #[serde(default)]
    pub blocking_gaps: Vec<String>,
    /// Time-based stop-loss deadline, e.g. "业绩说明会后10个交易日"
    #[serde(default)]
    pub time_stop_deadline: String,
    /// Reason for the time stop, e.g. "催化剂落空后回归纯现金观察"
    #[serde(default)]
    pub time_stop_reason: String,
    #[serde(default, skip_serializing)]
    pub markdown: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StructuredPortfolioDecision {
    pub rating: Rating,
    #[serde(default)]
    pub raw_rating: String,
    #[serde(default)]
    pub calibrated_rating: String,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub confidence: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub risk_assessment: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub executive_summary: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub investment_thesis: LocalText,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub rationale: LocalText,
    pub price_target: String,
    #[serde(default)]
    pub confirmation_level: String,
    #[serde(default)]
    pub invalidation_level: String,
    #[serde(default)]
    pub target_type: String,
    #[serde(default)]
    pub target_reference: String,
    #[serde(default)]
    pub target_condition: String,
    pub time_horizon: String,
    #[serde(default)]
    pub missing_evidence_ladder: MissingEvidenceLadder,
    #[serde(default)]
    pub trigger_checklist: Vec<String>,
    #[serde(default, skip_serializing)]
    pub markdown: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MissingEvidenceLadder {
    #[serde(default)]
    pub tolerable_gaps: Vec<String>,
    #[serde(default)]
    pub manageable_gaps: Vec<String>,
    #[serde(default)]
    pub blocking_gaps: Vec<String>,
}


/// Event-driven catalyst scoring card for evaluating earnings calls, guidance, etc.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CatalystScoreCard {
    /// Name of the catalyst event, e.g. "2025Q2业绩说明会"
    #[serde(default)]
    pub event_name: String,
    /// Scoring items: each is a (question, score: 0 or 1) pair
    #[serde(default)]
    pub items: Vec<CatalystScoreItem>,
    /// Total score across all items
    #[serde(default)]
    pub total_score: i32,
    /// Maximum possible score
    #[serde(default)]
    pub max_score: i32,
    /// Interpretation of the total score, e.g. "积极 — 上调关注度"
    #[serde(default)]
    pub interpretation: LocalText,
    /// Recommended next action based on score
    #[serde(default)]
    pub recommended_action: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CatalystScoreItem {
    /// The evaluation question, e.g. "管理层是否明确指引毛利率改善？"
    #[serde(default)]
    pub question: LocalText,
    /// Score: 1 if yes, 0 if no
    #[serde(default)]
    pub score: i32,
    /// Optional evidence or notes
    #[serde(default)]
    pub evidence: LocalText,
}

/// Daily and weekly review checklist for ongoing monitoring
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReviewChecklist {
    /// Daily review items (check after market close)
    #[serde(default)]
    pub daily: Vec<ReviewItem>,
    /// Weekly review items (check on weekends)
    #[serde(default)]
    pub weekly: Vec<ReviewItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReviewItem {
    /// What to check, e.g. "价格是否接近61.75或66.0？"
    #[serde(default)]
    pub check: LocalText,
    /// Category: price, technical, fundamental, discipline
    #[serde(default)]
    pub category: String,
    /// Priority: high, medium, low
    #[serde(default)]
    pub priority: String,
}

#[cfg(test)]
mod reflection_plan_tests {
    use super::*;

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
}

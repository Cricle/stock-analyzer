use stock_analyzer::analysis::derive_setup_tags;
use stock_analyzer::analysis::{
    ConfidenceBreakdown, DirectionBreakdown, ExecutionReadiness, LocalText, ScoreDimension,
    SignedScoreDimension, StructuredPortfolioDecision, StructuredResearchPlan,
    StructuredTraderPlan,
};

fn default_inputs() -> (
    ConfidenceBreakdown,
    DirectionBreakdown,
    ExecutionReadiness,
    StructuredResearchPlan,
    StructuredTraderPlan,
    StructuredPortfolioDecision,
) {
    (
        ConfidenceBreakdown::default(),
        DirectionBreakdown::default(),
        ExecutionReadiness::default(),
        StructuredResearchPlan::default(),
        StructuredTraderPlan::default(),
        StructuredPortfolioDecision::default(),
    )
}

#[test]
fn default_inputs_produce_watchlist_only() {
    let (conf, dir, exec, research, trader, portfolio) = default_inputs();
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(tags.contains(&"watchlist_only".to_string()));
    assert!(!tags.contains(&"execution_ready".to_string()));
}

#[test]
fn execution_boundary_complete_produces_execution_ready() {
    let (conf, dir, mut exec, research, trader, portfolio) = default_inputs();
    exec.execution_boundary_complete = true;
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(tags.contains(&"execution_ready".to_string()));
    assert!(!tags.contains(&"watchlist_only".to_string()));
}

#[test]
fn trend_confirmed_tag() {
    let (mut conf, mut dir, exec, research, trader, portfolio) = default_inputs();
    conf.trend_confirmation = ScoreDimension {
        score: 15,
        max_score: 20,
        rationale: LocalText::default(),
    };
    dir.market = SignedScoreDimension {
        score: 12,
        min_score: -25,
        max_score: 25,
        rationale: LocalText::default(),
    };
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(tags.contains(&"trend_confirmed".to_string()));
}

#[test]
fn trend_not_confirmed_low_score() {
    let (mut conf, dir, exec, research, trader, portfolio) = default_inputs();
    conf.trend_confirmation = ScoreDimension {
        score: 5,
        max_score: 20,
        rationale: LocalText::default(),
    };
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(!tags.contains(&"trend_confirmed".to_string()));
}

#[test]
fn event_driven_from_catalyst_quality() {
    let (mut conf, dir, exec, research, trader, portfolio) = default_inputs();
    conf.catalyst_quality = ScoreDimension {
        score: 10,
        max_score: 15,
        rationale: LocalText::default(),
    };
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(tags.contains(&"event_driven".to_string()));
}

#[test]
fn event_driven_from_trigger_checklist() {
    let (conf, dir, exec, research, trader, mut portfolio) = default_inputs();
    portfolio.trigger_checklist = vec!["earnings".into()];
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(tags.contains(&"event_driven".to_string()));
}

#[test]
fn event_driven_from_execution_trigger_checklist() {
    let (conf, dir, exec, research, mut trader, portfolio) = default_inputs();
    trader.execution_trigger_checklist = vec!["vol".into()];
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(tags.contains(&"event_driven".to_string()));
}

#[test]
fn fundamental_quality_tag() {
    let (mut conf, dir, exec, research, trader, portfolio) = default_inputs();
    conf.fundamental_confirmation = ScoreDimension {
        score: 15,
        max_score: 20,
        rationale: LocalText::default(),
    };
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(tags.contains(&"fundamental_quality".to_string()));
}

#[test]
fn valuation_sensitive_from_manageable_gaps() {
    let (conf, dir, exec, mut research, trader, portfolio) = default_inputs();
    research.missing_evidence_ladder.manageable_gaps = vec!["gap1".into()];
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(tags.contains(&"valuation_sensitive".to_string()));
}

#[test]
fn valuation_sensitive_from_blocking_gaps() {
    let (conf, dir, exec, research, trader, mut portfolio) = default_inputs();
    portfolio.missing_evidence_ladder.blocking_gaps = vec!["gap1".into()];
    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    assert!(tags.contains(&"valuation_sensitive".to_string()));
}

#[test]
fn no_duplicate_tags() {
    let (mut conf, mut dir, mut exec, mut research, mut trader, mut portfolio) = default_inputs();
    // Trigger multiple conditions that could produce duplicates
    conf.trend_confirmation.score = 15;
    dir.market.score = 12;
    conf.catalyst_quality.score = 10;
    portfolio.trigger_checklist = vec!["t".into()];
    trader.execution_trigger_checklist = vec!["t".into()];
    conf.fundamental_confirmation.score = 15;
    portfolio.missing_evidence_ladder.blocking_gaps = vec!["g".into()];
    research.missing_evidence_ladder.manageable_gaps = vec!["g".into()];
    exec.execution_boundary_complete = true;

    let tags = derive_setup_tags(&conf, &dir, &exec, &research, &trader, &portfolio);
    // Check no duplicates
    let unique: std::collections::HashSet<&String> = tags.iter().collect();
    assert_eq!(
        tags.len(),
        unique.len(),
        "tags should be unique: {:?}",
        tags
    );
}

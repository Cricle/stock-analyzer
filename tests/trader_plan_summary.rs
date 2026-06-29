use sa::{
    CoreResearchCall, DecisionAction, DecisionView, Rating, StructuredPortfolioDecision,
    StructuredTraderPlan, is_publishable_summary_reference,
};

#[test]
fn publishable_summary_reference_filters_template_fragments() {
    assert!(!is_publishable_summary_reference("确认后再评估上行空间"));
    assert!(!is_publishable_summary_reference(
        "升级为可执行看多需同时满足：财务口径确认完整"
    ));
    assert!(is_publishable_summary_reference("311.4上方有效突破"));
    assert!(is_publishable_summary_reference("跌破270.55"));
}

#[test]
fn authoritative_summary_skips_unpublishable_confirmation_and_target_fragments() {
    let summary = StructuredPortfolioDecision {
        rating: Rating::Hold,
        confirmation_level: "升级为可执行看多需同时满足：财务口径确认完整".to_string(),
        invalidation_level: "若补齐数据后显示价格明显弱于50日均线".to_string(),
        target_reference: "确认后再评估上行空间".to_string(),
        investment_thesis: "基本面质量仍在".into(),
        risk_assessment: "风险可控".into(),
        ..Default::default()
    }
    .authoritative_summary(
        &StructuredTraderPlan {
            action: "Hold".into(),
            ..Default::default()
        },
        40,
        &CoreResearchCall::Neutral,
        &DecisionView {
            action: DecisionAction::Hold,
            ..Default::default()
        },
    );

    assert!(!summary.contains("当前最值得盯住的确认位在"));
    assert!(!summary.contains("目标参考先看"));
    assert!(!summary.contains("若出现 若补齐数据后显示"));
}

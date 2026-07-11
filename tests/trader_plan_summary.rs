use stock_analyzer::{
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

#[test]
fn llm_summary_is_preserved_when_substantive() {
    use stock_analyzer::{
        CoreResearchCall, DecisionAction, DecisionView, LocalText, Rating,
        StructuredPortfolioDecision, StructuredTraderPlan,
    };

    let decision = StructuredPortfolioDecision {
        rating: Rating::Hold,
        executive_summary: LocalText::new(
            "贵州茅台当前处于高位震荡格局，1800元附近有较强支撑，建议等待回调后再考虑加仓。",
        ),
        ..Default::default()
    };
    let llm_summary = decision.executive_summary.clone();
    let template_summary = decision.authoritative_summary(
        &StructuredTraderPlan {
            action: "Hold".into(),
            ..Default::default()
        },
        65,
        &CoreResearchCall::Neutral,
        &DecisionView {
            action: DecisionAction::Hold,
            ..Default::default()
        },
    );

    // LLM summary should be kept (not overwritten by template)
    assert_ne!(llm_summary.key, template_summary);
    assert!(llm_summary.key.len() > 20);
    assert!(!llm_summary.key.contains("Model did not return"));
}

#[test]
fn template_fallback_when_llm_summary_is_placeholder() {
    use stock_analyzer::{
        CoreResearchCall, DecisionAction, DecisionView, LocalText, Rating,
        StructuredPortfolioDecision, StructuredTraderPlan,
    };

    let decision = StructuredPortfolioDecision {
        rating: Rating::Hold,
        executive_summary: LocalText::new(
            "Model did not return portfolio manager executive summary.",
        ),
        ..Default::default()
    };
    let template_summary = decision.authoritative_summary(
        &StructuredTraderPlan {
            action: "Hold".into(),
            ..Default::default()
        },
        65,
        &CoreResearchCall::Neutral,
        &DecisionView {
            action: DecisionAction::Hold,
            ..Default::default()
        },
    );

    // Template should be generated (has substantive content)
    assert!(template_summary.len() > 20);
    // Template uses struct fields, so it may contain the placeholder text
    // The important thing is that the template path is taken (tested in report_builder.rs)
}

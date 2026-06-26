use sa::analysis::is_publishable_summary_reference;
use sa::analysis::{
    TechnicalValues, derive_action_guides, derive_memory_reference_facts,
    derive_news_diagnostics, derive_news_insights, derive_report_diagnostics,
    derive_setup_match_explanation, derive_technical_conclusions,
    detect_disclosure_sequence_complexity, is_semantically_similar,
};
use sa::{
    AgentStateSnapshot, AnalysisArtifacts,
    AnalysisResult, AnalysisScenarioContext, AnalysisScenarioData, AnalysisScenarioIssue,
    AnalystRuntimeState, CalibrationProfile, CandlePoint,
    ConfidenceCap, ConfidenceProfile, CoreResearchCall, DecisionAction, DecisionConfidenceBand,
    DecisionView, DecisionViewDirection, HistoricalMemoryHighlight, LocalText,
    MemoryContextSnapshot, MissingEvidenceLadder, NewsInsight, PriceContext, Rating,
    ReferenceFactItem, ReportDiagnosticItem, ReportDiagnostics,
    ReportReferenceSnapshot, StructuredPortfolioDecision,
    StructuredReflection, StructuredResearchPlan, StructuredRiskAssessment, StructuredTraderPlan,
    TechnicalIndicatorConclusion, ToolObservation,
};
use serde_json::json;

// ===== tests from setup_news.rs =====

#[test]
fn setup_explanation_zero_history_is_explicitly_cautious() {
    let explanation = derive_setup_match_explanation(&MemoryContextSnapshot::default(), 0);
    assert_eq!(explanation.reason_code, "setup_filter_not_used");

    let explanation = derive_setup_match_explanation(
        &MemoryContextSnapshot {
            used_setup_filtered_retrieval: true,
            ..Default::default()
        },
        0,
    );
    assert_eq!(explanation.reason_code, "no_matching_setup_history");
    assert!(explanation.summary.contains("历史部分只能弱参考"));
    assert!(explanation.summary.contains("主要依赖当期证据"));
}

#[test]
fn setup_explanation_distinguishes_pending_strict_matches_from_verified_fallback() {
    let explanation = derive_setup_match_explanation(
        &MemoryContextSnapshot {
            used_setup_filtered_retrieval: true,
            setup_pending_match_count: 2,
            setup_calibration_sample_count: 4,
            used_setup_fallback_calibration: true,
            ..Default::default()
        },
        4,
    );
    assert_eq!(
        explanation.reason_code,
        "pending_only_with_verified_fallback_samples"
    );
    assert!(explanation.fallback_used);
    assert_eq!(explanation.fallback_sample_count, 4);
}

#[test]
fn watcher_guide_calls_out_unverified_setup_history() {
    let result = AnalysisResult {
        task_id: "task-1".to_string(),
        report_id: "report-1".to_string(),
        symbol: "603629".to_string(),
        stock_name: "demo".to_string(),
        analysis_date: "2026-05-14".to_string(),
        market_type: "CN".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: Default::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-05-14T00:00:00Z".to_string(),
    };
    let guides = derive_action_guides(
        &result,
        &StructuredResearchPlan::default(),
        &StructuredTraderPlan::default(),
        &StructuredPortfolioDecision {
            rating: Rating::Hold,
            ..Default::default()
        },
        &ConfidenceProfile::default(),
        &[ConfidenceCap {
            key: "zero_resolved_setup_history".to_string(),
            ..Default::default()
        }],
    );
    assert_eq!(guides.watchers.summary.key, "summary_watchers_weak");
    assert!(
        guides
            .watchers
            .actions
            .iter()
            .any(|item| item.key == "action_watcher_weak_history")
    );
}

#[test]
fn semantic_similarity_detects_repeated_summary_variants() {
    let left = "Final stance: Hold. Execution action: Hold. Objective confidence: 10/100.";
    let right = "Final stance: Hold. Execution action: Hold. Objective confidence: 10/100";
    assert!(is_semantically_similar(
        Some(&left.to_string()),
        Some(&right.to_string())
    ));
}

#[test]
fn derive_news_insights_assigns_distinct_structured_interpretations() {
    let references = ReportReferenceSnapshot {
        news: vec![
            ReferenceFactItem {
                key: "news_item".to_string(),
                value: "Structured market update".to_string(),
                summary: "Neutral update".to_string(),
                emphasis: "Reuters".to_string(),
                url: "https://example.com/approval".to_string(),
                label: "2026-05-19".to_string(),
            },
            ReferenceFactItem {
                key: "news_item".to_string(),
                value: "Regulatory filing reference".to_string(),
                summary: "Filing context".to_string(),
                emphasis: "SEC".to_string(),
                url: "https://www.sec.gov/Archives/example".to_string(),
                label: "2026-05-18".to_string(),
            },
        ],
        ..Default::default()
    };
    let decision = DecisionView {
        view: DecisionViewDirection::Neutral,
        action: DecisionAction::Hold,
        confidence_band: DecisionConfidenceBand::Low,
        primary_path: "Path A".to_string(),
        next_upgrade_condition: LocalText::new("next_upgrade_with_confirmation")
            .with_str("level", "Need approval plus price confirmation"),
        next_downgrade_condition: LocalText::new("next_downgrade_with_invalidation")
            .with_str("invalidation", "Break support"),
        ..Default::default()
    };

    let insights: Vec<NewsInsight> = derive_news_insights(
        &references,
        &decision,
        &PriceContext {
            distance_to_high_pct: Some(1.2),
            ..Default::default()
        },
        &ReportDiagnostics::default(),
        "2026-06-04",
    );
    assert_eq!(insights.len(), 2);
    assert_eq!(
        insights[0].interpretation,
        "news_needs_price_confirmation_after_catalyst".into()
    );
    assert_eq!(insights[1].interpretation, "news_reference_only".into());
    assert_ne!(insights[0].interpretation, insights[1].interpretation);
}

#[test]
fn technical_conclusions_combine_strong_trend_with_fading_momentum() {
    let conclusions: Vec<TechnicalIndicatorConclusion> = derive_technical_conclusions(
        &TechnicalValues {
            adx: Some(36.0),
            macd_hist: Some(-1.2),
            ema10: Some(98.0),
            ..Default::default()
        },
        Some(101.0),
    );
    assert!(
        conclusions
            .iter()
            .any(|item| item.key == "trend_strength_with_fading_momentum")
    );
}

#[test]
fn structured_risk_assessment_prefers_structured_sections() {
    let risk = StructuredRiskAssessment::from_text(
        "decision_blocking_gaps: 缺量价确认; 缺财务跟进\nkey_risks: 破位; 兑现压力\noffsetting_supports: 趋势未破\noverall_risk_framing: 当前应先观察而不是升级风险",
    );
    assert_eq!(risk.decision_blocking_gaps.len(), 2);
    assert_eq!(risk.key_risks.len(), 2);
    assert_eq!(risk.offsetting_supports, vec!["趋势未破".to_string()]);
    assert_eq!(risk.overall_risk_framing, "当前应先观察而不是升级风险");
}

#[test]
fn structured_reflection_keeps_raw_input_and_structured_fields() {
    let reflection = StructuredReflection::from_text(
        "{\"strongest_part\":\"证据链完整\",\"weakest_uncertainty\":\"催化不足\",\"next_lesson\":\"优先补财务拆解\"}",
    );
    assert_eq!(reflection.strengths, "证据链完整".into());
    assert_eq!(reflection.uncertainties, "催化不足".into());
    assert_eq!(reflection.next_lessons, "优先补财务拆解".into());
    assert!(reflection.raw_reflection.contains("strongest_part"));
}

#[test]
fn disclosure_sequence_complexity_detects_clustered_capital_markets_filings() {
    let result = AnalysisResult {
        task_id: "task-1".to_string(),
        report_id: "report-1".to_string(),
        symbol: "TEST".to_string(),
        stock_name: "test".to_string(),
        analysis_date: "2026-05-21".to_string(),
        market_type: "US".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: Default::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-05-21T00:00:00Z".to_string(),
    };
    let diagnostic = detect_disclosure_sequence_complexity(
        &result,
        &[
            ReferenceFactItem {
                key: "news_item".to_string(),
                label: "2026-05-12".to_string(),
                value: "Company files 424B5 prospectus supplement".to_string(),
                summary: "Registration filing for securities resale.".to_string(),
                emphasis: "SEC".to_string(),
                url: "https://www.sec.gov/Archives/example-1".to_string(),
                ..Default::default()
            },
            ReferenceFactItem {
                key: "news_item".to_string(),
                label: "2026-05-19".to_string(),
                value: "SEC Form 144 filed for proposed sale of securities".to_string(),
                summary: "Potential insider sale registration notice.".to_string(),
                emphasis: "SEC".to_string(),
                url: "https://www.sec.gov/Archives/example-2".to_string(),
                ..Default::default()
            },
            ReferenceFactItem {
                key: "news_item".to_string(),
                label: "2026-05-15".to_string(),
                value: "Company files 8-K update".to_string(),
                summary: "No direct business catalyst disclosed.".to_string(),
                emphasis: "SEC".to_string(),
                url: "https://www.sec.gov/Archives/example-3".to_string(),
                ..Default::default()
            },
        ],
    )
    .expect("expected disclosure sequence diagnostic");

    assert_eq!(diagnostic.code, "disclosure_sequence_complexity");
}

#[test]
fn derive_news_insights_downgrades_reference_filings_when_disclosure_sequence_is_complex() {
    let references = ReportReferenceSnapshot {
        news: vec![ReferenceFactItem {
            key: "news_item".to_string(),
            value: "SEC Form 144 filed for proposed sale of securities".to_string(),
            summary: "Potential insider sale registration notice.".to_string(),
            emphasis: "SEC".to_string(),
            url: "https://example.com/form144".to_string(),
            label: "2026-05-19".to_string(),
        }],
        ..Default::default()
    };
    let decision = DecisionView::default();
    let diagnostics = ReportDiagnostics {
        news: vec![ReportDiagnosticItem {
            code: "disclosure_sequence_complexity".to_string(),
            severity: "warning".to_string(),
            message: "complexity".into(),
            details: vec![],
            ..Default::default()
        }],
        ..Default::default()
    };

    let insights = derive_news_insights(
        &references,
        &decision,
        &PriceContext::default(),
        &diagnostics,
        "2026-06-04",
    );

    assert_eq!(insights.len(), 1);
    assert_eq!(
        insights[0].interpretation,
        "news_disclosure_sequence_needs_context".into()
    );
    assert_eq!(insights[0].impact_direction, "caution".into());
    assert_eq!(
        insights[0].what_to_watch_next,
        "watch_disclosure_overhang_resolution".into()
    );
}

#[test]
fn memory_reference_facts_include_structured_history_highlights() {
    let facts = derive_memory_reference_facts(
        &Default::default(),
        &MemoryContextSnapshot {
            historical_same_ticker_highlights: vec![HistoricalMemoryHighlight {
                trade_date: "2026-05-20".to_string(),
                ticker: "NVDA".to_string(),
                summary: "利润增长但现金流未跟上".to_string(),
                key_risk: "现金流背离".to_string(),
                lesson: "先拆营运资本再升级结论".to_string(),
                ..Default::default()
            }],
            historical_cross_ticker_highlights: vec![HistoricalMemoryHighlight {
                trade_date: "2026-05-18".to_string(),
                ticker: "AMD".to_string(),
                summary: "突破后回踩确认更健康".to_string(),
                key_risk: "追价失败".to_string(),
                lesson: "等待量价确认".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        },
    );

    assert!(facts.iter().any(|item| item.key == "same_ticker_history"));
    assert!(facts.iter().any(|item| item.key == "cross_ticker_lesson"));
}

// ===== tests from news_diagnostics.rs =====

#[test]
fn news_diagnostics_flag_source_concentration_and_weak_fetch_coverage() {
    let result = AnalysisResult {
        task_id: "task-1".to_string(),
        report_id: "report-1".to_string(),
        symbol: "TEST".to_string(),
        stock_name: "test".to_string(),
        analysis_date: "2026-05-21".to_string(),
        market_type: "US".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: AnalysisArtifacts {
            analyst_runtime_states: vec![AnalystRuntimeState {
                key: "news".to_string(),
                tool_history: vec![ToolObservation {
                    tool_name: "get_news".to_string(),
                    success: true,
                    meta: json!({
                        "sources": ["SearxNG"],
                        "attempts": [
                            { "source": "SearxNG", "success": false, "item_count": 0 },
                            { "source": "SearxNG", "success": false, "item_count": 0 },
                            { "source": "SearxNG", "success": true, "item_count": 1 }
                        ]
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        },
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-05-21T00:00:00Z".to_string(),
    };

    let diagnostics = derive_news_diagnostics(&result);
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == "news_source_concentration")
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == "news_fetch_coverage_weak")
    );
}

#[test]
fn availability_diagnostics_include_scenario_issues_and_market_minimums() {
    let result = AnalysisResult {
        task_id: "task-2".to_string(),
        report_id: "report-2".to_string(),
        symbol: "TEST-HK".to_string(),
        stock_name: "Test HK".to_string(),
        analysis_date: "2026-05-21".to_string(),
        market_type: "港股".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: AnalysisArtifacts {
            scenario_context: AnalysisScenarioContext::from_market_type("港股"),
            scenario_data: AnalysisScenarioData {
                quote_status: "missing".to_string(),
                candles_status: "ok".to_string(),
                fundamentals_status: "missing".to_string(),
                company_news_status: "sparse".to_string(),
                issues: vec![AnalysisScenarioIssue {
                    domain: "quote".to_string(),
                    code: "quote_missing".to_string(),
                    severity: "warning".to_string(),
                    message: "quote prefetch missing for test symbol".to_string(),
                }],
                candles: vec![CandlePoint {
                    trade_date: "2026-05-20".to_string(),
                    open: 1.0,
                    close: 1.0,
                    high: 1.0,
                    low: 1.0,
                    volume: 1,
                    amount: 1.0,
                    amplitude_pct: 0.0,
                    change_pct: 0.0,
                    change_amount: 0.0,
                    turnover_pct: 0.0,
                }],
                ..Default::default()
            },
            ..Default::default()
        },
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-05-21T00:00:00Z".to_string(),
    };

    let diagnostics = derive_report_diagnostics(&result);
    assert!(
        diagnostics
            .availability
            .iter()
            .any(|item| item.code == "quote_missing")
    );
    assert!(
        diagnostics
            .availability
            .iter()
            .any(|item| item.code == "scenario_minimum_hk_equity_incomplete")
    );
}

#[test]
fn scenario_minimum_errors_become_execution_blocking_gaps() {
    let mut result = AnalysisResult {
        task_id: "task-3".to_string(),
        report_id: "report-3".to_string(),
        symbol: "TEST-US".to_string(),
        stock_name: "Test US".to_string(),
        analysis_date: "2026-05-21".to_string(),
        market_type: "美股".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: AnalysisArtifacts {
            scenario_context: AnalysisScenarioContext::from_market_type("美股"),
            scenario_data: AnalysisScenarioData {
                quote_status: "ok".to_string(),
                candles_status: "ok".to_string(),
                fundamentals_status: "missing".to_string(),
                company_news_status: "missing".to_string(),
                quote: Some(sa::types::QuoteSnapshot {
                    symbol: "TEST-US".to_string(),
                    date: "2026-05-20".to_string(),
                    open: 1.0,
                    high: 1.0,
                    low: 1.0,
                    close: 1.0,
                    volume: 1,
                }),
                candles: vec![CandlePoint {
                    trade_date: "2026-05-20".to_string(),
                    open: 1.0,
                    close: 1.0,
                    high: 1.0,
                    low: 1.0,
                    volume: 1,
                    amount: 1.0,
                    amplitude_pct: 0.0,
                    change_pct: 0.0,
                    change_amount: 0.0,
                    turnover_pct: 0.0,
                }],
                ..Default::default()
            },
            ..Default::default()
        },
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-05-21T00:00:00Z".to_string(),
    };

    result.rebuild_report(&CalibrationProfile::default());
    assert!(
        result
            .report
            .portfolio_decision
            .missing_evidence_ladder
            .blocking_gaps
            .iter()
            .any(|item| item.contains("scenario_minimum_incomplete"))
    );
}

#[test]
fn availability_diagnostics_include_related_gap_linkage() {
    let mut result = AnalysisResult {
        task_id: "task-3b".to_string(),
        report_id: "report-3b".to_string(),
        symbol: "TEST-US-2".to_string(),
        stock_name: "Test US 2".to_string(),
        analysis_date: "2026-05-21".to_string(),
        market_type: "美股".to_string(),
        graph: Default::default(),
        agent_state: AgentStateSnapshot {
            structured_research_plan: StructuredResearchPlan {
                missing_evidence_ladder: MissingEvidenceLadder {
                    blocking_gaps: vec!["缺 fundamentals 与 company_news 双确认".to_string()],
                    ..Default::default()
                },
                trigger_checklist: vec!["等待 company_news 回暖后再确认".to_string()],
                ..Default::default()
            },
            structured_trader_plan: StructuredTraderPlan {
                execution_trigger_checklist: vec!["company_news 恢复且量价确认".to_string()],
                blocking_gaps: vec!["company_news 缺失时不执行".to_string()],
                ..Default::default()
            },
            structured_portfolio_decision: StructuredPortfolioDecision {
                missing_evidence_ladder: MissingEvidenceLadder {
                    blocking_gaps: vec!["缺 fundamentals 导致不能升级仓位".to_string()],
                    ..Default::default()
                },
                trigger_checklist: vec!["fundamentals 补齐后再评估".to_string()],
                ..Default::default()
            },
            ..Default::default()
        },
        artifacts: AnalysisArtifacts {
            scenario_context: AnalysisScenarioContext::from_market_type("美股"),
            scenario_data: AnalysisScenarioData {
                quote_status: "ok".to_string(),
                candles_status: "ok".to_string(),
                fundamentals_status: "missing".to_string(),
                company_news_status: "missing".to_string(),
                quote: Some(sa::types::QuoteSnapshot {
                    symbol: "TEST-US-2".to_string(),
                    date: "2026-05-20".to_string(),
                    open: 1.0,
                    high: 1.0,
                    low: 1.0,
                    close: 1.0,
                    volume: 1,
                }),
                candles: vec![CandlePoint {
                    trade_date: "2026-05-20".to_string(),
                    open: 1.0,
                    close: 1.0,
                    high: 1.0,
                    low: 1.0,
                    volume: 1,
                    amount: 1.0,
                    amplitude_pct: 0.0,
                    change_pct: 0.0,
                    change_amount: 0.0,
                    turnover_pct: 0.0,
                }],
                ..Default::default()
            },
            ..Default::default()
        },
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-05-21T00:00:00Z".to_string(),
    };

    result.rebuild_report(&CalibrationProfile::default());
    let scenario_gap = result
        .report
        .diagnostics
        .availability
        .iter()
        .find(|item| item.code == "scenario_minimum_us_equity_incomplete")
        .expect("scenario minimum diagnostic");
    assert!(scenario_gap.elevated_to_execution_blocking_gap);
    assert!(!scenario_gap.related_blocking_gaps.is_empty());
    assert!(!scenario_gap.related_trigger_checklist.is_empty());
}

#[test]
fn scenario_minimum_errors_are_injected_into_executive_summary() {
    let mut result = AnalysisResult {
        task_id: "task-4".to_string(),
        report_id: "report-4".to_string(),
        symbol: "TEST-US-3".to_string(),
        stock_name: "Test US 3".to_string(),
        analysis_date: "2026-05-21".to_string(),
        market_type: "美股".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: AnalysisArtifacts {
            scenario_context: AnalysisScenarioContext::from_market_type("美股"),
            scenario_data: AnalysisScenarioData {
                quote_status: "ok".to_string(),
                candles_status: "ok".to_string(),
                fundamentals_status: "missing".to_string(),
                company_news_status: "missing".to_string(),
                quote: Some(sa::types::QuoteSnapshot {
                    symbol: "TEST-US-3".to_string(),
                    date: "2026-05-20".to_string(),
                    open: 1.0,
                    high: 1.0,
                    low: 1.0,
                    close: 1.0,
                    volume: 1,
                }),
                candles: vec![CandlePoint {
                    trade_date: "2026-05-20".to_string(),
                    open: 1.0,
                    close: 1.0,
                    high: 1.0,
                    low: 1.0,
                    volume: 1,
                    amount: 1.0,
                    amplitude_pct: 0.0,
                    change_pct: 0.0,
                    change_amount: 0.0,
                    turnover_pct: 0.0,
                }],
                ..Default::default()
            },
            ..Default::default()
        },
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-05-21T00:00:00Z".to_string(),
    };

    result.rebuild_report(&CalibrationProfile::default());
    assert!(
        result
            .report
            .portfolio_decision
            .executive_summary
            .contains("当前不能升级结论的直接原因是")
    );
    assert!(
        result
            .report
            .portfolio_decision
            .executive_summary
            .contains("scenario_minimum_incomplete")
    );
}

// ===== tests from summary.rs =====

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

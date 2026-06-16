    use super::{
        AnalysisResult, ConfidenceProfile, MemoryContextSnapshot,
        MissingEvidenceLadder,
        NewsInsight, PriceContext, Rating, ReferenceFactItem, ReportDiagnosticItem,
        ReportDiagnostics, ReportReferenceSnapshot, StructuredPortfolioDecision,
        StructuredReflection, StructuredResearchPlan, StructuredRiskAssessment,
        StructuredTraderPlan, TechnicalIndicatorConclusion, derive_action_guides,
        derive_news_insights, derive_memory_reference_facts, derive_news_diagnostics,
        derive_report_diagnostics, derive_setup_match_explanation,
        derive_technical_conclusions, detect_disclosure_sequence_complexity,
        is_semantically_similar, DecisionAction, DecisionConfidenceBand, DecisionView,
        DecisionViewDirection, LocalText, TechnicalValues,
    };
    use crate::models::{HistoricalMemoryHighlight, ToolObservation};
    use serde_json::json;

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
        assert!(
            explanation
                .summary
                .contains("主要依赖当期证据")
        );
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
            next_upgrade_condition: LocalText::new("next_upgrade_with_confirmation").with_str("level", "Need approval plus price confirmation"),
            next_downgrade_condition: LocalText::new("next_downgrade_with_invalidation").with_str("invalidation", "Break support"),
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
        assert!(conclusions
            .iter()
            .any(|item| item.key == "trend_strength_with_fading_momentum"));
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
        let diagnostic = detect_disclosure_sequence_complexity(&result, &[
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
        ])
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
        assert_eq!(insights[0].interpretation, "news_disclosure_sequence_needs_context".into());
        assert_eq!(insights[0].impact_direction, "caution".into());
        assert_eq!(insights[0].what_to_watch_next, "watch_disclosure_overhang_resolution".into());
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

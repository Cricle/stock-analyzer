
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
            artifacts: crate::AnalysisArtifacts {
                analyst_runtime_states: vec![crate::AnalystRuntimeState {
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
        assert!(diagnostics.iter().any(|item| item.code == "news_source_concentration"));
        assert!(diagnostics.iter().any(|item| item.code == "news_fetch_coverage_weak"));
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
            artifacts: crate::AnalysisArtifacts {
                scenario_context: crate::AnalysisScenarioContext::from_market_type("港股"),
                scenario_data: crate::AnalysisScenarioData {
                    quote_status: "missing".to_string(),
                    candles_status: "ok".to_string(),
                    fundamentals_status: "missing".to_string(),
                    company_news_status: "sparse".to_string(),
                    issues: vec![crate::AnalysisScenarioIssue {
                        domain: "quote".to_string(),
                        code: "quote_missing".to_string(),
                        severity: "warning".to_string(),
                        message: "quote prefetch missing for test symbol".to_string(),
                    }],
                    candles: vec![sa_types::CandlePoint {
                        trade_date: "2026-05-20".to_string(),
                        open: Decimal::ONE,
                        close: Decimal::ONE,
                        high: Decimal::ONE,
                        low: Decimal::ONE,
                        volume: 1,
                        amount: Decimal::ONE,
                        amplitude_pct: 0.0,
                        change_pct: 0.0,
                        change_amount: Decimal::ZERO,
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
        assert!(diagnostics
            .availability
            .iter()
            .any(|item| item.code == "quote_missing"));
        assert!(diagnostics
            .availability
            .iter()
            .any(|item| item.code == "scenario_minimum_hk_equity_incomplete"));
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
            artifacts: crate::AnalysisArtifacts {
                scenario_context: crate::AnalysisScenarioContext::from_market_type("美股"),
                scenario_data: crate::AnalysisScenarioData {
                    quote_status: "ok".to_string(),
                    candles_status: "ok".to_string(),
                    fundamentals_status: "missing".to_string(),
                    company_news_status: "missing".to_string(),
                    quote: Some(sa_types::QuoteSnapshot {
                        symbol: "TEST-US".to_string(),
                        date: "2026-05-20".to_string(),
                        open: Decimal::ONE,
                        high: Decimal::ONE,
                        low: Decimal::ONE,
                        close: Decimal::ONE,
                        volume: 1,
                    }),
                    candles: vec![sa_types::CandlePoint {
                        trade_date: "2026-05-20".to_string(),
                        open: Decimal::ONE,
                        close: Decimal::ONE,
                        high: Decimal::ONE,
                        low: Decimal::ONE,
                        volume: 1,
                        amount: Decimal::ONE,
                        amplitude_pct: 0.0,
                        change_pct: 0.0,
                        change_amount: Decimal::ZERO,
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

        result.rebuild_report(&crate::scoring::CalibrationProfile::default());
        assert!(result
            .report
            .portfolio_decision
            .missing_evidence_ladder
            .blocking_gaps
            .iter()
            .any(|item| item.contains("scenario_minimum_incomplete")));
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
            agent_state: crate::AgentStateSnapshot {
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
            artifacts: crate::AnalysisArtifacts {
                scenario_context: crate::AnalysisScenarioContext::from_market_type("美股"),
                scenario_data: crate::AnalysisScenarioData {
                    quote_status: "ok".to_string(),
                    candles_status: "ok".to_string(),
                    fundamentals_status: "missing".to_string(),
                    company_news_status: "missing".to_string(),
                    quote: Some(sa_types::QuoteSnapshot {
                        symbol: "TEST-US-2".to_string(),
                        date: "2026-05-20".to_string(),
                        open: Decimal::ONE,
                        high: Decimal::ONE,
                        low: Decimal::ONE,
                        close: Decimal::ONE,
                        volume: 1,
                    }),
                    candles: vec![sa_types::CandlePoint {
                        trade_date: "2026-05-20".to_string(),
                        open: Decimal::ONE,
                        close: Decimal::ONE,
                        high: Decimal::ONE,
                        low: Decimal::ONE,
                        volume: 1,
                        amount: Decimal::ONE,
                        amplitude_pct: 0.0,
                        change_pct: 0.0,
                        change_amount: Decimal::ZERO,
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

        result.rebuild_report(&crate::scoring::CalibrationProfile::default());
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
            artifacts: crate::AnalysisArtifacts {
                scenario_context: crate::AnalysisScenarioContext::from_market_type("美股"),
                scenario_data: crate::AnalysisScenarioData {
                    quote_status: "ok".to_string(),
                    candles_status: "ok".to_string(),
                    fundamentals_status: "missing".to_string(),
                    company_news_status: "missing".to_string(),
                    quote: Some(sa_types::QuoteSnapshot {
                        symbol: "TEST-US-3".to_string(),
                        date: "2026-05-20".to_string(),
                        open: Decimal::ONE,
                        high: Decimal::ONE,
                        low: Decimal::ONE,
                        close: Decimal::ONE,
                        volume: 1,
                    }),
                    candles: vec![sa_types::CandlePoint {
                        trade_date: "2026-05-20".to_string(),
                        open: Decimal::ONE,
                        close: Decimal::ONE,
                        high: Decimal::ONE,
                        low: Decimal::ONE,
                        volume: 1,
                        amount: Decimal::ONE,
                        amplitude_pct: 0.0,
                        change_pct: 0.0,
                        change_amount: Decimal::ZERO,
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

        result.rebuild_report(&crate::scoring::CalibrationProfile::default());
        assert!(result
            .report
            .portfolio_decision
            .executive_summary
            .contains("当前不能升级结论的直接原因是"));
        assert!(result
            .report
            .portfolio_decision
            .executive_summary
            .contains("scenario_minimum_incomplete"));
    }

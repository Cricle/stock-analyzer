use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{Duration, Utc};

use stock_analyzer::{
    AnalysisResult, AnalysisStore, InMemoryAnalysisStore, InMemoryCacheStore,
    InMemoryCheckpointStore, QuoteSnapshot, TaskRunParams, TaskStatus,
    checkpoint::{TaskCheckpoint, TaskCheckpointStore},
    report::lifecycle::{
        DataDomain, DataProvenance, ReportDataAvailability, ReportQualityGate,
        evaluate_report_quality_gate,
    },
};

#[test]
fn blocked_statuses_round_trip_and_are_terminal() {
    assert_eq!(
        "blocked_data".parse::<TaskStatus>().unwrap(),
        TaskStatus::BlockedData
    );
    assert_eq!(TaskStatus::BlockedLlm.as_str(), "blocked_llm");
    assert!(TaskStatus::BlockedData.is_terminal());
    assert!(TaskStatus::BlockedLlm.is_terminal());
}

#[test]
fn gate_blocks_missing_fundamentals() {
    let gate = evaluate_report_quality_gate(&ReportDataAvailability {
        quote: true,
        candle_count: 300,
        fundamentals: false,
        company_news_count: 2,
        provenance: BTreeMap::new(),
    });

    assert!(!gate.passed);
    assert_eq!(gate.blocking_domains, vec![DataDomain::Fundamentals]);
}

#[test]
fn gate_requires_minimum_candle_history() {
    let gate = evaluate_report_quality_gate(&ReportDataAvailability {
        quote: true,
        candle_count: 59,
        fundamentals: true,
        company_news_count: 1,
        provenance: BTreeMap::new(),
    });

    assert_eq!(gate.blocking_domains, vec![DataDomain::Candles]);
}

#[test]
fn fallback_provider_is_preserved_as_provenance() {
    let mut provenance = BTreeMap::new();
    provenance.insert(
        DataDomain::Fundamentals,
        DataProvenance::successful("finnhub", None, 1, false),
    );
    let gate = ReportQualityGate::from_availability(ReportDataAvailability {
        quote: true,
        candle_count: 300,
        fundamentals: true,
        company_news_count: 1,
        provenance,
    });

    assert_eq!(gate.provenance["fundamentals"].provider, "finnhub");
}

#[test]
fn empty_company_news_is_blocking() {
    let gate = evaluate_report_quality_gate(&ReportDataAvailability {
        quote: true,
        candle_count: 300,
        fundamentals: true,
        company_news_count: 0,
        provenance: BTreeMap::new(),
    });

    assert_eq!(gate.blocking_domains, vec![DataDomain::CompanyNews]);
}

#[test]
fn stale_cached_data_without_source_timestamp_is_blocking() {
    let mut provenance = BTreeMap::new();
    provenance.insert(
        DataDomain::Quote,
        DataProvenance::successful("redis_cache", None, 1, true),
    );
    let gate = evaluate_report_quality_gate(&ReportDataAvailability {
        quote: true,
        candle_count: 300,
        fundamentals: true,
        company_news_count: 1,
        provenance,
    });

    assert_eq!(gate.blocking_domains, vec![DataDomain::Quote]);
}

#[test]
fn cached_data_restores_persisted_provenance_before_passing_the_gate() {
    let source_timestamp = Some(Utc::now().to_rfc3339());
    let persisted = ReportQualityGate::from_availability(ReportDataAvailability {
        quote: true,
        candle_count: 300,
        fundamentals: true,
        company_news_count: 1,
        provenance: BTreeMap::from([
            (
                DataDomain::Quote,
                DataProvenance::successful("primary_quote", source_timestamp.clone(), 1, true),
            ),
            (
                DataDomain::Candles,
                DataProvenance::successful("primary_candles", source_timestamp.clone(), 300, true),
            ),
            (
                DataDomain::Fundamentals,
                DataProvenance::successful("primary_fundamentals", None, 1, false),
            ),
            (
                DataDomain::CompanyNews,
                DataProvenance::successful("primary_news", None, 1, false),
            ),
        ]),
    });

    let restored = ReportQualityGate::from_cached_availability(
        ReportDataAvailability {
            quote: true,
            candle_count: 300,
            fundamentals: true,
            company_news_count: 1,
            provenance: BTreeMap::new(),
        },
        Some(&persisted),
    );

    assert!(restored.passed);
    assert_eq!(restored.provenance["quote"].provider, "primary_quote");
}

#[test]
fn cached_data_without_persisted_provenance_is_conservatively_blocked() {
    let gate = ReportQualityGate::from_cached_availability(
        ReportDataAvailability {
            quote: true,
            candle_count: 300,
            fundamentals: true,
            company_news_count: 1,
            provenance: BTreeMap::new(),
        },
        None,
    );

    assert!(!gate.passed);
    assert_eq!(
        gate.blocking_domains,
        vec![
            DataDomain::Quote,
            DataDomain::Candles,
            DataDomain::Fundamentals,
            DataDomain::CompanyNews,
        ]
    );
}

#[test]
fn cached_data_with_expired_source_timestamp_is_blocked() {
    let mut provenance = BTreeMap::new();
    provenance.insert(
        DataDomain::Quote,
        DataProvenance::successful(
            "redis_cache",
            Some((Utc::now() - Duration::days(8)).to_rfc3339()),
            1,
            true,
        ),
    );
    let gate = evaluate_report_quality_gate(&ReportDataAvailability {
        quote: true,
        candle_count: 300,
        fundamentals: true,
        company_news_count: 1,
        provenance,
    });

    assert_eq!(gate.blocking_domains, vec![DataDomain::Quote]);
}

#[test]
fn persisted_quality_gate_is_recovered_from_fetch_diagnosis() {
    let persisted = ReportQualityGate::from_availability(ReportDataAvailability {
        quote: true,
        candle_count: 300,
        fundamentals: true,
        company_news_count: 1,
        provenance: BTreeMap::from([(
            DataDomain::Quote,
            DataProvenance::successful("primary_quote", None, 1, false),
        )]),
    });
    let diagnosis = vec![
        serde_json::json!({"source": "unrelated"}),
        serde_json::to_value(&persisted).unwrap(),
    ];

    let restored = ReportQualityGate::from_fetch_diagnosis(&diagnosis).unwrap();

    assert!(restored.passed);
    assert_eq!(restored.provenance["quote"].provider, "primary_quote");
}

#[test]
fn resumed_gate_failure_persists_blocked_task_and_progress_event_before_llm() {
    std::thread::Builder::new()
        .name("quality-gate-lifecycle-test".to_string())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(
                    resumed_gate_failure_persists_blocked_task_and_progress_event_before_llm_async(
                    ),
                )
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn resumed_gate_failure_persists_blocked_task_and_progress_event_before_llm_async() {
    let data_dir = tempfile::tempdir().unwrap();
    let analysis_store: Arc<dyn AnalysisStore> = Arc::new(InMemoryAnalysisStore::new());
    let checkpoint_store = TaskCheckpointStore::new(Arc::new(InMemoryCheckpointStore::new()));
    let manager = stock_analyzer::TaskManager::new(
        analysis_store,
        Arc::new(InMemoryCacheStore::new()),
        None,
        None,
        stock_analyzer::MarketDataClient::new().await.unwrap(),
        data_dir.path().to_str().unwrap().to_string(),
        stock_analyzer::memory::TradingMemoryLog::new(data_dir.path().to_str().unwrap(), 10)
            .unwrap(),
        checkpoint_store,
        1,
        1,
        stock_analyzer::telemetry::init_telemetry(),
    )
    .await
    .unwrap();
    let task_id = manager
        .create_task_with_id(
            "quality-gate-test",
            stock_analyzer::SingleAnalysisRequest {
                symbol: Some("AAPL".to_string()),
                stock_code: None,
                stock_name: Some("Apple".to_string()),
                parameters: Some(stock_analyzer::AnalysisParameters {
                    market_type: Some("US".to_string()),
                    analysis_date: Some("2026-07-21".to_string()),
                    ..Default::default()
                }),
                force_refresh: true,
            },
            Some("quality-gate-resume".to_string()),
            false,
        )
        .await
        .unwrap();
    let task = manager
        .analysis_store()
        .get_task(&task_id)
        .await
        .unwrap()
        .unwrap();
    let mut result: AnalysisResult = serde_json::from_value(serde_json::json!({
        "task_id": task_id,
        "report_id": "report-quality-gate-resume",
        "symbol": "AAPL",
        "stock_name": "Apple",
        "analysis_date": "2026-07-21",
        "market_type": "US",
        "created_at": Utc::now().to_rfc3339(),
    }))
    .unwrap();
    result.artifacts.scenario_data.quote = Some(QuoteSnapshot {
        symbol: "AAPL".to_string(),
        date: "2026-07-21".to_string(),
        open: 200.0,
        high: 202.0,
        low: 199.0,
        close: 201.0,
        volume: 1_000,
    });
    manager
        .checkpoint_store
        .save(&TaskCheckpoint {
            task_id: task_id.clone(),
            symbol: task.symbol.clone(),
            analysis_date: task.analysis_date.clone(),
            stage: "market".to_string(),
            node: "market".to_string(),
            result,
            step: 1,
        })
        .await
        .unwrap();

    let mut events = manager.subscribe(&task_id).await;
    manager
        .execute_existing_task(
            task_id.clone(),
            TaskRunParams::for_reflection("2026-07-21".to_string(), "en"),
        )
        .await
        .unwrap();

    let blocked_event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let event = events.recv().await.unwrap();
            if event.status == TaskStatus::BlockedData {
                return event;
            }
        }
    })
    .await
    .expect("resumed gate failure should publish a terminal progress event");
    let persisted_task = manager
        .analysis_store()
        .get_task(&task_id)
        .await
        .unwrap()
        .unwrap();
    let persisted_gate: ReportQualityGate = serde_json::from_value(
        persisted_task
            .quality_gate_json
            .clone()
            .expect("evaluated quality gate should be persisted"),
    )
    .unwrap();

    assert_eq!(persisted_task.status, TaskStatus::BlockedData);
    assert!(persisted_task.status.is_terminal());
    assert_eq!(blocked_event.status, TaskStatus::BlockedData);
    assert_eq!(blocked_event.event_type, "progress_update");
    assert!(!persisted_gate.passed);
    assert!(
        persisted_gate
            .blocking_domains
            .contains(&DataDomain::Fundamentals)
    );
}

#[test]
fn provenance_retains_primary_and_empty_fallback_attempts() {
    let mut provenance = DataProvenance::successful("market_data", None, 1, false);
    provenance.record_failed_attempt("finnhub", "empty response");

    assert_eq!(provenance.attempts.len(), 2);
    assert_eq!(provenance.attempts[0]["provider"], "market_data");
    assert_eq!(provenance.attempts[0]["success"], true);
    assert_eq!(provenance.attempts[1]["provider"], "finnhub");
    assert_eq!(provenance.attempts[1]["success"], false);
    assert_eq!(provenance.attempts[1]["error"], "empty response");
}

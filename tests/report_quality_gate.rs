use std::collections::BTreeMap;

use chrono::{Duration, Utc};

use stock_analyzer::{
    TaskStatus,
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

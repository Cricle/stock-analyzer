use std::collections::BTreeMap;

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

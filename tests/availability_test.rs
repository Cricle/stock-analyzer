use stock_analyzer::analysis::cashflow_snapshot_missing;

#[test]
fn missing_all_cashflow_fields_is_a_fundamental_evidence_gap() {
    assert!(cashflow_snapshot_missing(None, None, None));
    assert!(!cashflow_snapshot_missing(Some(1.0), None, None));
}

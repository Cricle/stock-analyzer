use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DataProvenance {
    pub source: String,
    pub fetched_at: String,
    pub confidence: f64,
    pub field_coverage: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProvenanceSnapshot {
    pub market_data: Option<DataProvenance>,
    pub fundamentals: Option<DataProvenance>,
    pub technicals: Option<DataProvenance>,
    pub news: Option<DataProvenance>,
}

/// Score source quality: primary sources = 5, secondary = 3, unknown = 0
fn score_source_quality(source: &str) -> i32 {
    match source {
        "tushare" | "finnhub" => 5,
        "yahoo" | "akshare" | "gdelt" => 3,
        "computed_from_candles" => 5,
        _ => 0,
    }
}

/// Score data freshness: <24h = 5, <7d = 3, <30d = 1, older = 0
fn score_freshness(fetched_at: &str) -> i32 {
    let Ok(fetched) = chrono::DateTime::parse_from_rfc3339(fetched_at) else {
        return 0;
    };
    let now = chrono::Utc::now();
    let age = now.signed_duration_since(fetched.with_timezone(&chrono::Utc));

    if age.num_hours() < 24 {
        5
    } else if age.num_days() < 7 {
        3
    } else if age.num_days() < 30 {
        1
    } else {
        0
    }
}

/// Score field coverage: all critical = 5, ≥80% = 3, ≥50% = 1
#[allow(dead_code)]
fn score_coverage(field_coverage: &[String], expected_fields: &[&str]) -> i32 {
    if expected_fields.is_empty() {
        return 0;
    }
    let coverage_pct = field_coverage.len() as f64 / expected_fields.len() as f64;
    if coverage_pct >= 1.0 {
        5
    } else if coverage_pct >= 0.8 {
        3
    } else if coverage_pct >= 0.5 {
        1
    } else {
        0
    }
}

/// Score confidence: ≥0.8 = 5, ≥0.6 = 3, ≥0.4 = 1
fn score_confidence(confidence: f64) -> i32 {
    if confidence >= 0.8 {
        5
    } else if confidence >= 0.6 {
        3
    } else if confidence >= 0.4 {
        1
    } else {
        0
    }
}

/// Score overall provenance snapshot (0-20 points)
pub fn score_provenance(snapshot: &ProvenanceSnapshot) -> i32 {
    let mut total = 0;
    let mut count = 0;

    // Average scores across available sources
    if let Some(ref prov) = snapshot.market_data {
        total += score_source_quality(&prov.source);
        total += score_freshness(&prov.fetched_at);
        total += score_confidence(prov.confidence);
        count += 1;
    }

    if let Some(ref prov) = snapshot.fundamentals {
        total += score_source_quality(&prov.source);
        total += score_freshness(&prov.fetched_at);
        total += score_confidence(prov.confidence);
        count += 1;
    }

    if let Some(ref prov) = snapshot.technicals {
        total += score_source_quality(&prov.source);
        total += score_confidence(prov.confidence);
        count += 1;
    }

    if let Some(ref prov) = snapshot.news {
        total += score_source_quality(&prov.source);
        total += score_freshness(&prov.fetched_at);
        count += 1;
    }

    if count == 0 {
        return 0;
    }

    // Normalize to 0-20
    ((total as f64 / count as f64) * (20.0 / 15.0)).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_data_provenance() {
        let prov = DataProvenance {
            source: "tushare".to_string(),
            fetched_at: "2026-07-20T10:00:00Z".to_string(),
            confidence: 0.95,
            field_coverage: vec!["price".to_string()],
        };
        assert_eq!(prov.source, "tushare");
        assert_eq!(prov.confidence, 0.95);
    }

    #[test]
    fn test_score_source_quality() {
        assert_eq!(score_source_quality("tushare"), 5);
        assert_eq!(score_source_quality("finnhub"), 5);
        assert_eq!(score_source_quality("yahoo"), 3);
        assert_eq!(score_source_quality("akshare"), 3);
        assert_eq!(score_source_quality("unknown"), 0);
    }

    #[test]
    fn test_score_freshness() {
        let now = chrono::Utc::now();
        let fresh = now.to_rfc3339();
        let week_old = (now - chrono::Duration::days(7)).to_rfc3339();
        let month_old = (now - chrono::Duration::days(31)).to_rfc3339();

        assert_eq!(score_freshness(&fresh), 5);
        assert_eq!(score_freshness(&week_old), 1); // 7 days = boundary, scores 1 not 3
        assert_eq!(score_freshness(&month_old), 0);
    }

    #[test]
    fn test_score_provenance_full() {
        let snapshot = ProvenanceSnapshot {
            market_data: Some(DataProvenance {
                source: "tushare".to_string(),
                fetched_at: chrono::Utc::now().to_rfc3339(),
                confidence: 0.95,
                field_coverage: vec!["price".to_string(), "volume".to_string()],
            }),
            fundamentals: None,
            technicals: None,
            news: None,
        };

        let score = score_provenance(&snapshot);
        assert!(score >= 0 && score <= 20);
    }
}

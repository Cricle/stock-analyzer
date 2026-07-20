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
}

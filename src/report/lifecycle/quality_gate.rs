use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::data::{FundamentalsSnapshot, NewsItem, QuoteSnapshot};
use crate::{DataFetchDiagnosis, ReportCandle};

pub const MINIMUM_CANDLE_COUNT: usize = 60;
const MAX_STALE_CACHE_AGE_DAYS: i64 = 7;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataDomain {
    Quote,
    Candles,
    Fundamentals,
    CompanyNews,
}

impl DataDomain {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Candles => "candles",
            Self::Fundamentals => "fundamentals",
            Self::CompanyNews => "company_news",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "quote" => Some(Self::Quote),
            "candles" => Some(Self::Candles),
            "fundamentals" => Some(Self::Fundamentals),
            "company_news" => Some(Self::CompanyNews),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DataProvenance {
    pub provider: String,
    pub fetched_at: String,
    pub source_timestamp: Option<String>,
    pub record_count: usize,
    pub used_cache: bool,
    pub attempts: Vec<serde_json::Value>,
}

impl DataProvenance {
    pub fn successful(
        provider: impl Into<String>,
        source_timestamp: Option<String>,
        record_count: usize,
        used_cache: bool,
    ) -> Self {
        let provider = provider.into();
        let fetched_at = Utc::now().to_rfc3339();
        let mut provenance = Self {
            provider: provider.clone(),
            source_timestamp: cache_validation_timestamp(
                source_timestamp,
                record_count,
                used_cache,
                &fetched_at,
            ),
            fetched_at,
            record_count,
            used_cache,
            attempts: Vec::new(),
        };
        provenance.record_successful_attempt(provider);
        provenance
    }

    pub fn from_diagnosis(
        diagnosis: &DataFetchDiagnosis,
        source_timestamp: Option<String>,
        record_count: usize,
    ) -> Self {
        let provider = diagnosis
            .attempts
            .iter()
            .rev()
            .find(|attempt| attempt.success)
            .map(|attempt| attempt.provider.clone())
            .unwrap_or_else(|| "unavailable".to_string());
        let fetched_at = Utc::now().to_rfc3339();
        Self {
            provider,
            source_timestamp: cache_validation_timestamp(
                source_timestamp,
                record_count,
                diagnosis.used_stale_cache,
                &fetched_at,
            ),
            fetched_at,
            record_count,
            used_cache: diagnosis.used_stale_cache,
            attempts: diagnosis
                .attempts
                .iter()
                .map(|attempt| serde_json::to_value(attempt).unwrap_or_default())
                .collect(),
        }
    }

    pub fn from_attempts(
        provider: impl Into<String>,
        source_timestamp: Option<String>,
        record_count: usize,
        used_cache: bool,
        attempts: Vec<serde_json::Value>,
    ) -> Self {
        let fetched_at = Utc::now().to_rfc3339();
        Self {
            provider: provider.into(),
            source_timestamp: cache_validation_timestamp(
                source_timestamp,
                record_count,
                used_cache,
                &fetched_at,
            ),
            fetched_at,
            record_count,
            used_cache,
            attempts,
        }
    }

    pub fn failed(provider: impl Into<String>, error: impl Into<String>) -> Self {
        let provider = provider.into();
        let mut provenance = Self {
            provider: provider.clone(),
            fetched_at: Utc::now().to_rfc3339(),
            attempts: Vec::new(),
            ..Self::default()
        };
        provenance.record_failed_attempt(provider, error);
        provenance
    }

    pub fn record_successful_attempt(&mut self, provider: impl Into<String>) {
        self.attempts.push(serde_json::json!({
            "provider": provider.into(),
            "success": true,
        }));
    }

    pub fn record_failed_attempt(&mut self, provider: impl Into<String>, error: impl Into<String>) {
        self.attempts.push(serde_json::json!({
            "provider": provider.into(),
            "success": false,
            "error": error.into(),
        }));
    }
}

fn cache_validation_timestamp(
    source_timestamp: Option<String>,
    record_count: usize,
    used_cache: bool,
    fetched_at: &str,
) -> Option<String> {
    source_timestamp.or_else(|| (!used_cache && record_count > 0).then(|| fetched_at.to_string()))
}

#[derive(Clone, Debug, Default)]
pub struct ReportDataAvailability {
    pub quote: bool,
    pub candle_count: usize,
    pub fundamentals: bool,
    pub company_news_count: usize,
    pub provenance: BTreeMap<DataDomain, DataProvenance>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportQualityGate {
    pub passed: bool,
    pub blocking_domains: Vec<DataDomain>,
    pub provenance: BTreeMap<String, DataProvenance>,
    pub checked_at: String,
}

impl ReportQualityGate {
    pub fn from_availability(availability: ReportDataAvailability) -> Self {
        evaluate_report_quality_gate(&availability)
    }

    pub fn from_fetch_diagnosis(fetch_diagnosis: &[serde_json::Value]) -> Option<Self> {
        fetch_diagnosis.iter().rev().find_map(|entry| {
            entry
                .get("passed")?
                .as_bool()
                .and_then(|_| entry.get("blocking_domains"))?
                .as_array()?;
            entry.get("provenance")?.as_object()?;
            serde_json::from_value(entry.clone()).ok()
        })
    }

    pub fn from_cached_availability(
        mut availability: ReportDataAvailability,
        persisted_gate: Option<&ReportQualityGate>,
    ) -> Self {
        availability.provenance = persisted_gate
            .map(|gate| {
                gate.provenance
                    .iter()
                    .filter_map(|(domain, provenance)| {
                        DataDomain::from_str(domain).map(|domain| {
                            let mut provenance = provenance.clone();
                            provenance.used_cache = true;
                            (domain, provenance)
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        evaluate_quality_gate(&availability, true)
    }

    pub fn from_acquired_data(
        quote: &Option<QuoteSnapshot>,
        candles: &[ReportCandle],
        fundamentals: &Option<FundamentalsSnapshot>,
        news_items: &[NewsItem],
        provenance: BTreeMap<DataDomain, DataProvenance>,
        checked_at: DateTime<Utc>,
    ) -> Self {
        let mut gate = evaluate_report_quality_gate(&ReportDataAvailability {
            quote: quote.is_some(),
            candle_count: candles.len(),
            fundamentals: fundamentals.is_some(),
            company_news_count: news_items.len(),
            provenance,
        });
        gate.checked_at = checked_at.to_rfc3339();
        gate
    }

    pub fn summary(&self) -> String {
        if self.passed {
            "All execution-critical report evidence is available".to_string()
        } else {
            format!(
                "Missing execution-critical evidence: {}",
                self.blocking_domains
                    .iter()
                    .map(DataDomain::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

pub fn evaluate_report_quality_gate(availability: &ReportDataAvailability) -> ReportQualityGate {
    evaluate_quality_gate(availability, false)
}

fn evaluate_quality_gate(
    availability: &ReportDataAvailability,
    require_provenance: bool,
) -> ReportQualityGate {
    let mut blocking_domains = Vec::new();
    if !domain_is_usable(
        DataDomain::Quote,
        availability.quote,
        &availability.provenance,
        require_provenance,
    ) {
        blocking_domains.push(DataDomain::Quote);
    }
    if !domain_is_usable(
        DataDomain::Candles,
        availability.candle_count >= MINIMUM_CANDLE_COUNT,
        &availability.provenance,
        require_provenance,
    ) {
        blocking_domains.push(DataDomain::Candles);
    }
    if !domain_is_usable(
        DataDomain::Fundamentals,
        availability.fundamentals,
        &availability.provenance,
        require_provenance,
    ) {
        blocking_domains.push(DataDomain::Fundamentals);
    }
    if !domain_is_usable(
        DataDomain::CompanyNews,
        availability.company_news_count > 0,
        &availability.provenance,
        require_provenance,
    ) {
        blocking_domains.push(DataDomain::CompanyNews);
    }

    let provenance = availability
        .provenance
        .iter()
        .map(|(domain, provenance)| (domain.as_str().to_string(), provenance.clone()))
        .collect();
    ReportQualityGate {
        passed: blocking_domains.is_empty(),
        blocking_domains,
        provenance,
        checked_at: Utc::now().to_rfc3339(),
    }
}

fn domain_is_usable(
    domain: DataDomain,
    available: bool,
    provenance: &BTreeMap<DataDomain, DataProvenance>,
    require_provenance: bool,
) -> bool {
    available
        && provenance.get(&domain).is_some_and(|source| {
            !source.used_cache || source_timestamp_is_fresh(source.source_timestamp.as_deref())
        })
        || (available && !require_provenance && !provenance.contains_key(&domain))
}

fn source_timestamp_is_fresh(source_timestamp: Option<&str>) -> bool {
    let Some(source_timestamp) = source_timestamp else {
        return false;
    };
    let source_date = DateTime::parse_from_rfc3339(source_timestamp)
        .map(|timestamp| timestamp.date_naive())
        .or_else(|_| NaiveDate::parse_from_str(source_timestamp, "%Y-%m-%d"));
    source_date.is_ok_and(|date| {
        let age = Utc::now()
            .date_naive()
            .signed_duration_since(date)
            .num_days();
        (0..=MAX_STALE_CACHE_AGE_DAYS).contains(&age)
    })
}

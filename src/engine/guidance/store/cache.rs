//! Trait-based caching for daily guidance reports.

use super::*;

impl GuidanceStore {
    fn cache_key(date: &str, market: &str) -> String {
        format!(
            "{GUIDANCE_CACHE_PREFIX}:report:{}:{}",
            date.trim(),
            market.trim().to_ascii_lowercase()
        )
    }

    fn stale_key(date: &str, market: &str) -> String {
        format!("{}:stale", Self::cache_key(date, market))
    }

    pub async fn get_cached_report(
        &self,
        date: &str,
        market: &str,
    ) -> Option<DailyGuidanceReport> {
        let key = Self::cache_key(date, market);
        if let Ok(Some(raw)) = self.cache.get(&key).await
            && let Ok(mut report) = serde_json::from_slice::<DailyGuidanceReport>(&raw)
        {
            report.metadata.cache_hit = true;
            return Some(report);
        }
        let stale = Self::stale_key(date, market);
        if let Ok(Some(raw)) = self.cache.get(&stale).await
            && let Ok(mut report) = serde_json::from_slice::<DailyGuidanceReport>(&raw)
        {
            report.metadata.cache_hit = true;
            return Some(report);
        }
        None
    }

    pub async fn cache_report(&self, report: &DailyGuidanceReport) {
        let Ok(payload) = serde_json::to_vec(report) else {
            return;
        };
        let key = Self::cache_key(&report.date, &report.market);
        let stale = Self::stale_key(&report.date, &report.market);
        let _ = self
            .cache
            .set(&key, &payload, Some(GUIDANCE_CACHE_TTL_SECS))
            .await;
        let _ = self
            .cache
            .set(&stale, &payload, Some(GUIDANCE_STALE_TTL_SECS))
            .await;
    }

    /// Fetch the latest stock pick summary for a market.
    pub async fn get_latest_stock_pick_summary(
        &self,
        market: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let prefix = format!("{}:pick:{}", GUIDANCE_CACHE_PREFIX, market);
        let entries = self.cache.list_entries(&prefix).await?;
        if entries.is_empty() {
            return Ok(None);
        }
        // Find the most recent entry
        if let Some(entry) = entries.iter().max_by_key(|e| e.created_at.as_str())
            && let Ok(Some(bytes)) = self.cache.get(&entry.key).await
            && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            return Ok(Some(val));
        }
        Ok(None)
    }
}

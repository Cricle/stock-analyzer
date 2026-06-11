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

    fn prepared_key(date: &str, market: &str) -> String {
        format!(
            "{GUIDANCE_CACHE_PREFIX}:prepared:{}:{}",
            date.trim(),
            market.trim().to_ascii_lowercase()
        )
    }

    /// Store pre-fetched data for later report assembly.
    pub async fn store_prepared(&self, data: &PreparedData) {
        let Ok(payload) = serde_json::to_vec(data) else {
            return;
        };
        let key = Self::prepared_key(&data.date, &data.market);
        // 30 min TTL — prepared data is transient
        let _ = self.cache.set(&key, &payload, Some(30 * 60)).await;
    }

    /// Retrieve pre-fetched data.
    pub async fn get_prepared(&self, date: &str, market: &str) -> Option<PreparedData> {
        let key = Self::prepared_key(date, market);
        if let Ok(Some(raw)) = self.cache.get(&key).await {
            serde_json::from_slice(&raw).ok()
        } else {
            None
        }
    }

    /// Fetch the latest stock pick summary for a market.
    ///
    /// TODO: This previously used Redis SCAN which is not available via CacheStore trait.
    /// The CacheStore `list_entries` method could be used with a prefix filter instead.
    pub async fn get_latest_stock_pick_summary(
        &self,
        _market: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        // TODO: Implement using CacheStore::list_entries with prefix matching
        Ok(None)
    }
}

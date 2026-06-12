use anyhow::Context;

use super::{DataError, DataErrorKind, MarketDataClient, wire};

impl MarketDataClient {
    pub(super) async fn tushare_query(
        &self,
        api_name: &str,
        params: serde_json::Value,
        fields: &str,
    ) -> anyhow::Result<Vec<wire::TushareRow>> {
        let token = self.tushare_token.as_deref().ok_or_else(|| {
            DataError::new(
                DataErrorKind::MissingCredentials,
                "tushare token missing; set TUSHARE_TOKEN",
            )
        })?;
        let response = self
            .http
            .post("https://api.tushare.pro")
            .json(&serde_json::json!({
                "api_name": api_name,
                "token": token,
                "params": params,
                "fields": fields
            }))
            .send()
            .await
            .with_context(|| format!("failed to call tushare api {api_name}"))?
            .error_for_status()
            .with_context(|| format!("tushare http request failed for {api_name}"))?;

        let payload: wire::TushareResponse = response
            .json()
            .await
            .with_context(|| format!("failed to decode tushare response for {api_name}"))?;
        if payload.code != 0 {
            let kind = match payload.code {
                40203 => DataErrorKind::PermissionDenied,
                _ => DataErrorKind::Upstream,
            };
            return Err(DataError::new(
                kind,
                format!(
                    "tushare api {} failed with code {}: {}",
                    api_name,
                    payload.code,
                    payload.msg.unwrap_or_default()
                ),
            )
            .into());
        }
        let data = payload
            .data
            .context("tushare response missing data payload")?;
        Ok(data
            .items
            .into_iter()
            .map(|item| wire::TushareRow::new(&data.fields, item))
            .collect())
    }
}

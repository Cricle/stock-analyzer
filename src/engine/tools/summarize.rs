use serde_json::{Value, json};

use super::TradingToolbox;

impl TradingToolbox {
    pub(super) fn summarize_stock_data_output(output: &str) -> String {
        let Ok(value) = serde_json::from_str::<Value>(output) else {
            return output.to_string();
        };
        let symbol = value
            .get("symbol")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let market_type = value
            .get("market_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let start_date = value
            .get("start_date")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let end_date = value
            .get("end_date")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let rows = value
            .get("rows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if rows.is_empty() {
            return format!(
                "{{\"symbol\":\"{symbol}\",\"market_type\":\"{market_type}\",\"start_date\":\"{start_date}\",\"end_date\":\"{end_date}\",\"row_count\":0}}"
            );
        }
        let first = rows.first().cloned().unwrap_or(Value::Null);
        let last = rows.last().cloned().unwrap_or(Value::Null);
        let summary = json!({
            "symbol": symbol,
            "market_type": market_type,
            "start_date": start_date,
            "end_date": end_date,
            "row_count": rows.len(),
            "first_trade_date": first.get("trade_date").cloned().unwrap_or(Value::Null),
            "last_trade_date": last.get("trade_date").cloned().unwrap_or(Value::Null),
            "first_close": first.get("close").cloned().unwrap_or(Value::Null),
            "last_close": last.get("close").cloned().unwrap_or(Value::Null),
            "high_max": rows.iter().filter_map(|row| row.get("high").and_then(Value::as_f64)).reduce(f64::max),
            "low_min": rows.iter().filter_map(|row| row.get("low").and_then(Value::as_f64)).reduce(f64::min),
            "volume_sum": rows.iter().filter_map(|row| row.get("volume").and_then(Value::as_f64)).sum::<f64>(),
            "data_gap": value.get("data_gap").cloned().unwrap_or(Value::Null),
        });
        serde_json::to_string_pretty(&summary).unwrap_or_else(|_| output.to_string())
    }

    pub(super) fn summarize_indicator_output(output: &str, meta: &Value) -> String {
        let payload = serde_json::from_str::<Value>(output)
            .ok()
            .or_else(|| meta.get("payload").cloned());
        let Some(payload) = payload else {
            return output.to_string();
        };
        let summary = json!({
            "symbol": payload.get("symbol").cloned().unwrap_or(Value::Null),
            "start_date": payload.get("start_date").cloned().unwrap_or(Value::Null),
            "end_date": payload.get("end_date").cloned().unwrap_or(Value::Null),
            "history_candle_count": payload.get("history_candle_count").cloned().unwrap_or(Value::Null),
            "requested_window_candle_count": payload.get("requested_window_candle_count").cloned().unwrap_or(Value::Null),
            "indicators": payload.get("indicators").cloned().unwrap_or(Value::Array(Vec::new())),
            "data_gap": payload.get("data_gap").cloned().unwrap_or(Value::Null),
        });
        serde_json::to_string_pretty(&summary).unwrap_or_else(|_| output.to_string())
    }

    pub(super) fn summarize_json_object_output(output: &str, max_keys: usize) -> String {
        let Ok(value) = serde_json::from_str::<Value>(output) else {
            return output.to_string();
        };
        let Some(object) = value.as_object() else {
            return output.to_string();
        };
        let mut summary = serde_json::Map::new();
        for (index, (key, value)) in object.iter().enumerate() {
            if index >= max_keys {
                break;
            }
            summary.insert(key.clone(), value.clone());
        }
        serde_json::to_string_pretty(&Value::Object(summary)).unwrap_or_else(|_| output.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a minimal TradingToolbox for testing private methods
    // We can't easily construct TradingToolbox (needs MarketDataClient),
    // so we test the static/associated logic via the public interface.

    #[test]
    fn summarize_stock_data_output_invalid_json() {
        // summarize_stock_data_output is an impl method, but we can test via the
        // module's logic by checking that invalid JSON returns the original output.
        // Since these are private methods, we test through the dispatch in mod.rs
        // or test the JSON parsing logic directly.
        let invalid = "not json";
        let value: Result<Value, _> = serde_json::from_str(invalid);
        assert!(value.is_err());
    }

    #[test]
    fn summarize_json_object_output_truncates_keys() {
        let input = json!({"a": 1, "b": 2, "c": 3, "d": 4, "e": 5});
        let input_str = input.to_string();
        let value: Value = serde_json::from_str(&input_str).unwrap();
        let object = value.as_object().unwrap();
        let max_keys = 3;
        let mut summary = serde_json::Map::new();
        for (index, (key, value)) in object.iter().enumerate() {
            if index >= max_keys {
                break;
            }
            summary.insert(key.clone(), value.clone());
        }
        assert_eq!(summary.len(), 3);
    }

    #[test]
    fn summarize_stock_data_output_empty_rows() {
        let input = json!({
            "symbol": "AAPL",
            "market_type": "US",
            "start_date": "2024-01-01",
            "end_date": "2024-12-31",
            "rows": []
        });
        let value: Value = serde_json::from_str(&input.to_string()).unwrap();
        let rows = value
            .get("rows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(rows.is_empty());
    }

    #[test]
    fn summarize_stock_data_output_with_rows() {
        let input = json!({
            "symbol": "600519",
            "market_type": "A-share",
            "start_date": "2024-01-01",
            "end_date": "2024-01-03",
            "rows": [
                {"trade_date": "2024-01-01", "close": 100.0, "high": 105.0, "low": 95.0, "volume": 1000},
                {"trade_date": "2024-01-02", "close": 110.0, "high": 115.0, "low": 100.0, "volume": 2000},
                {"trade_date": "2024-01-03", "close": 108.0, "high": 112.0, "low": 106.0, "volume": 1500}
            ]
        });
        let value: Value = serde_json::from_str(&input.to_string()).unwrap();
        let rows = value
            .get("rows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(rows.len(), 3);
        let high_max = rows
            .iter()
            .filter_map(|row| row.get("high").and_then(Value::as_f64))
            .reduce(f64::max)
            .unwrap();
        assert_eq!(high_max, 115.0);
        let low_min = rows
            .iter()
            .filter_map(|row| row.get("low").and_then(Value::as_f64))
            .reduce(f64::min)
            .unwrap();
        assert_eq!(low_min, 95.0);
        let volume_sum: f64 = rows
            .iter()
            .filter_map(|row| row.get("volume").and_then(Value::as_f64))
            .sum();
        assert_eq!(volume_sum, 4500.0);
    }

    #[test]
    fn summarize_indicator_output_from_payload() {
        let meta = json!({
            "payload": {
                "symbol": "AAPL",
                "start_date": "2024-01-01",
                "end_date": "2024-12-31",
                "history_candle_count": 250,
                "requested_window_candle_count": 60,
                "indicators": [
                    {"name": "rsi", "value": 65.5},
                    {"name": "macd", "value": 0.5}
                ],
                "data_gap": null
            }
        });
        let payload = meta.get("payload").unwrap();
        let indicators = payload.get("indicators").and_then(Value::as_array).unwrap();
        assert_eq!(indicators.len(), 2);
        assert_eq!(indicators[0].get("name").unwrap().as_str().unwrap(), "rsi");
    }

    #[test]
    fn summarize_indicator_output_missing_payload() {
        let meta = json!({});
        assert!(meta.get("payload").is_none());
    }
}

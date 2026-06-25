use serde_json::Value;

pub fn summarize_tool_observation(item: &crate::types::ToolObservation) -> String {
    if !item.success {
        return item.output.clone();
    }

    match item.tool_name.as_str() {
        "get_stock_data" => summarize_stock_data_output(&item.output),
        "get_fundamentals" | "get_balance_sheet" | "get_cashflow" | "get_income_statement" => {
            summarize_json_kv_output(&item.output, 18)
        }
        "get_news" | "get_global_news" | "get_insider_transactions" => {
            summarize_news_output(&item.output)
        }
        _ => bounded_output(&item.output, 1800),
    }
}

pub fn summarize_stock_data_output(output: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(output) else {
        return bounded_output(output, 1800);
    };

    let symbol = value
        .get("symbol")
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
    if value.get("row_count").is_some() && value.get("rows").is_none() {
        let row_count = value
            .get("row_count")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let first_close = value
            .get("first_close")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let last_close = value
            .get("last_close")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let window_high = value
            .get("high_max")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let window_low = value
            .get("low_min")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let volume_sum = value
            .get("volume_sum")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let avg_volume = if row_count > 0 {
            volume_sum / row_count as f64
        } else {
            0.0
        };
        let pct_change = if first_close.abs() > f64::EPSILON {
            (last_close - first_close) / first_close * 100.0
        } else {
            0.0
        };
        return format!(
            "symbol: {symbol}\nwindow: {start_date} -> {end_date}\nrows: {row_count}\nfirst_close: {:.2}\nlast_close: {:.2}\nwindow_change_pct: {:.2}\nwindow_high: {:.2}\nwindow_low: {:.2}\navg_volume: {:.0}",
            first_close, last_close, pct_change, window_high, window_low, avg_volume,
        );
    }
    let rows = value
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if rows.is_empty() {
        return format!("symbol: {symbol}\nwindow: {start_date} -> {end_date}\nrows: 0");
    }

    let first = rows.first().cloned().unwrap_or(Value::Null);
    let last = rows.last().cloned().unwrap_or(Value::Null);
    let highs: Vec<f64> = rows
        .iter()
        .filter_map(|row| row.get("high").and_then(Value::as_f64))
        .collect();
    let lows: Vec<f64> = rows
        .iter()
        .filter_map(|row| row.get("low").and_then(Value::as_f64))
        .collect();
    let volumes: Vec<f64> = rows
        .iter()
        .filter_map(|row| row.get("volume").and_then(Value::as_f64))
        .collect();

    let close_first = first
        .get("close")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let close_last = last
        .get("close")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let pct_change = if close_first.abs() > f64::EPSILON {
        (close_last - close_first) / close_first * 100.0
    } else {
        0.0
    };
    let min_low = lows.iter().copied().fold(f64::INFINITY, f64::min);
    let max_high = highs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let avg_volume = if volumes.is_empty() {
        0.0
    } else {
        volumes.iter().sum::<f64>() / volumes.len() as f64
    };
    let recent = rows
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|row| {
            format!(
                "{} O:{:.2} H:{:.2} L:{:.2} C:{:.2} V:{:.0}",
                row.get("trade_date")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                row.get("open").and_then(Value::as_f64).unwrap_or_default(),
                row.get("high").and_then(Value::as_f64).unwrap_or_default(),
                row.get("low").and_then(Value::as_f64).unwrap_or_default(),
                row.get("close").and_then(Value::as_f64).unwrap_or_default(),
                row.get("volume")
                    .and_then(Value::as_f64)
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "symbol: {symbol}\nwindow: {start_date} -> {end_date}\nrows: {}\nfirst_close: {:.2}\nlast_close: {:.2}\nwindow_change_pct: {:.2}\nwindow_high: {:.2}\nwindow_low: {:.2}\navg_volume: {:.0}\nrecent_5_sessions:\n{}",
        rows.len(),
        close_first,
        close_last,
        pct_change,
        max_high,
        min_low,
        avg_volume,
        recent
    )
}

fn summarize_json_kv_output(output: &str, max_fields: usize) -> String {
    let Ok(value) = serde_json::from_str::<Value>(output) else {
        return bounded_output(output, 1800);
    };
    let Some(object) = value.as_object() else {
        return bounded_output(output, 1800);
    };

    object
        .iter()
        .take(max_fields)
        .map(|(key, value)| format!("{key}: {}", normalize_summary_value(value)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn summarize_news_output(output: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(output) else {
        return bounded_output(output, 1800);
    };

    let mut lines = Vec::new();
    if let Some(object) = value.as_object() {
        for key in [
            "symbol",
            "query",
            "curr_date",
            "start_date",
            "end_date",
            "look_back_days",
        ] {
            if let Some(raw) = object.get(key) {
                lines.push(format!("{key}: {}", normalize_summary_value(raw)));
            }
        }

        let items = object
            .get("items")
            .or_else(|| object.get("news"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        lines.push(format!("item_count: {}", items.len()));
        for (index, item) in items.into_iter().take(8).enumerate() {
            if let Some(map) = item.as_object() {
                let title = map
                    .get("title")
                    .or_else(|| map.get("headline"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let published = map
                    .get("published_at")
                    .or_else(|| map.get("datetime"))
                    .or_else(|| map.get("time"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let source = map
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let summary = map
                    .get("summary")
                    .or_else(|| map.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                lines.push(format!(
                    "{}. [{}] {} | {} | {}",
                    index + 1,
                    published,
                    source,
                    title,
                    summary.chars().take(180).collect::<String>()
                ));
            }
        }
    }

    if lines.is_empty() {
        bounded_output(output, 1800)
    } else {
        lines.join("\n")
    }
}

fn normalize_summary_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => format!("array(len={})", values.len()),
        Value::Object(values) => format!("object(keys={})", values.len()),
    }
}

fn bounded_output(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let truncated = text.chars().take(max_chars).collect::<String>();
    format!("{truncated}\n...[truncated]")
}

pub fn format_fundamental_metrics(sd: &crate::AnalysisScenarioData) -> String {
    let Some(f) = &sd.fundamentals else {
        return String::new();
    };
    let mut lines = Vec::new();
    let hundred = 100.0;
    if let (Some(mc), Some(ni)) = (f.market_cap, f.net_income_usd) {
        if ni > 0.0 {
            lines.push(format!("PE Ratio: {:.2}", mc / ni));
        } else {
            lines.push("PE Ratio: N/A (negative earnings)".to_string());
        }
    }
    if let (Some(mc), Some(eq)) = (f.market_cap, f.stockholders_equity_usd) {
        if eq.abs() > 0.0 {
            lines.push(format!("PB Ratio: {:.2}", mc / eq));
        }
    }
    if let (Some(gp), Some(rev)) = (f.gross_profit_usd, f.revenues_usd) {
        if rev.abs() > 0.0 {
            lines.push(format!("Gross Margin: {:.1}%", gp / rev * hundred));
        }
    }
    if let (Some(ni), Some(rev)) = (f.net_income_usd, f.revenues_usd) {
        if rev.abs() > 0.0 {
            lines.push(format!("Net Margin: {:.1}%", ni / rev * hundred));
        }
    }
    if let (Some(oi), Some(rev)) = (f.operating_income_usd, f.revenues_usd) {
        if rev.abs() > 0.0 {
            lines.push(format!("Operating Margin: {:.1}%", oi / rev * hundred));
        }
    }
    if let (Some(mc), Some(shares)) = (f.market_cap, f.shares_outstanding) {
        if shares > 0 {
            lines.push(format!("Market Cap: {:.0}", mc));
            lines.push(format!("Shares Outstanding: {}", shares));
        }
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("Fundamental Metrics:\n{}", lines.join("\n"))
    }
}

pub fn format_volume_profile(sd: &crate::AnalysisScenarioData) -> String {
    let candles = &sd.candles;
    if candles.len() < 5 {
        return String::new();
    }
    // Compute OBV (On-Balance Volume) trend
    let mid = candles.len() / 2;
    let mut obv = 0.0f64;
    let mut obv_at_mid = 0.0f64;
    for i in 1..candles.len() {
        let prev_close = candles[i - 1].close;
        let curr_close = candles[i].close;
        let vol = candles[i].volume as f64;
        if curr_close > prev_close {
            obv += vol;
        } else if curr_close < prev_close {
            obv -= vol;
        }
        if i == mid {
            obv_at_mid = obv;
        }
    }
    // Volume change: compare recent half vs earlier half
    let recent_avg: f64 =
        candles[mid..].iter().map(|c| c.volume as f64).sum::<f64>() / (candles.len() - mid) as f64;
    let earlier_avg: f64 = candles[..mid].iter().map(|c| c.volume as f64).sum::<f64>() / mid as f64;
    let volume_change_pct = if earlier_avg.abs() > f64::EPSILON {
        (recent_avg - earlier_avg) / earlier_avg * 100.0
    } else {
        0.0
    };
    let ad_signal = if obv > obv_at_mid {
        "accumulation"
    } else {
        "distribution"
    };
    format!(
        "Volume Profile:\nOBV Signal: {}\nVolume Change: {:.1}%",
        ad_signal, volume_change_pct
    )
}

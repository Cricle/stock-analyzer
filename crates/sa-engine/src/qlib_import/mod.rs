use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::Context;
use chrono::Utc;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::{fs, io::AsyncWriteExt, process::Command};
use uuid::Uuid;

const DEFAULT_QLIB_RELEASE_URL: &str = "https://gh-proxy.org/https://github.com/chenditc/investment_data/releases/download/2026-05-14/qlib_bin.tar.gz";
const DEFAULT_QLIB_COLLECTION: &str = "tradingagents_qlib_history";
const DEFAULT_CHUNK_SIZE: usize = 120;
const DEFAULT_BATCH_SIZE: usize = 64;
const DEFAULT_VECTOR_SIZE: usize = 384;

#[derive(Clone)]
struct QdrantBackend {
    http: reqwest::Client,
    url: String,
    collection: String,
}

#[derive(Clone, Debug)]
struct FeatureSeries {
    start_index: usize,
    values: Vec<f32>,
}

#[derive(Clone, Debug)]
struct QlibRow {
    trade_date: String,
    values: BTreeMap<String, f32>,
}

#[derive(Clone, Debug)]
struct QlibChunkDocument {
    ticker: String,
    market: String,
    start_date: String,
    end_date: String,
    text: String,
    payload: serde_json::Value,
}

pub async fn run_import(
    data_dir: &str,
    request: &sa_models::QlibImportRequest,
) -> anyhow::Result<sa_models::QlibImportResponse> {
    let started_at = Utc::now().to_rfc3339();
    let backend = build_qdrant_backend()?;
    ensure_collection(&backend).await?;

    let resolved = prepare_dataset(data_dir, request).await?;
    let qlib_root = detect_qlib_root(&resolved.dataset_root)?;
    let existing_points = collection_points(&backend).await?;
    let calendar = load_calendar(&qlib_root).await?;
    let chunk_size = request
        .chunk_size
        .unwrap_or(DEFAULT_CHUNK_SIZE)
        .clamp(20, 512);
    let symbol_limit = request.symbol_limit.filter(|value| *value > 0);

    if !request.force_reimport && symbol_limit.is_none() && existing_points > 0 {
        return Ok(sa_models::QlibImportResponse {
            started_at,
            finished_at: Utc::now().to_rfc3339(),
            collection: backend.collection,
            dataset_root: qlib_root.display().to_string(),
            release_url: resolved.release_url,
            archive_path: resolved.archive_path,
            downloaded: resolved.downloaded,
            skipped_existing_import: true,
            existing_points: Some(existing_points),
            skip_reason: Some(
                "qlib history collection already contains data; use force_reimport=true to rebuild"
                    .to_string(),
            ),
            chunk_size,
            symbol_limit,
            imported_symbols: 0,
            skipped_symbols: 0,
            imported_points: 0,
            sample_symbols: Vec::new(),
            embedding_provider: "hash".to_string(),
        });
    }

    let features_root = qlib_root.join("features");
    let mut entries = fs::read_dir(&features_root)
        .await
        .with_context(|| format!("failed to read {}", features_root.display()))?;

    let mut imported_symbols = 0usize;
    let mut skipped_symbols = 0usize;
    let mut imported_points = 0usize;
    let mut sample_symbols = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let Ok(file_type) = entry.file_type().await else {
            skipped_symbols += 1;
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        if symbol_limit.is_some_and(|limit| imported_symbols >= limit) {
            break;
        }

        let symbol_name = entry.file_name().to_string_lossy().to_string();
        match build_symbol_documents(&entry.path(), &symbol_name, &calendar, chunk_size).await {
            Ok(documents) if documents.is_empty() => {
                skipped_symbols += 1;
            }
            Ok(documents) => {
                if sample_symbols.len() < 8 {
                    sample_symbols.push(symbol_name.to_uppercase());
                }
                imported_points += documents.len();
                imported_symbols += 1;
                upsert_documents(&backend, &documents).await?;
            }
            Err(error) => {
                skipped_symbols += 1;
                tracing::warn!(
                    symbol = %symbol_name,
                    error = ?error,
                    "failed to import qlib symbol"
                );
            }
        }
    }

    Ok(sa_models::QlibImportResponse {
        started_at,
        finished_at: Utc::now().to_rfc3339(),
        collection: backend.collection,
        dataset_root: qlib_root.display().to_string(),
        release_url: resolved.release_url,
        archive_path: resolved.archive_path,
        downloaded: resolved.downloaded,
        skipped_existing_import: false,
        existing_points: Some(existing_points.saturating_add(imported_points)),
        skip_reason: None,
        chunk_size,
        symbol_limit,
        imported_symbols,
        skipped_symbols,
        imported_points,
        sample_symbols,
        embedding_provider: "hash".to_string(),
    })
}

pub async fn run_init_from_env(
    data_dir: &str,
) -> anyhow::Result<sa_models::QlibImportResponse> {
    let request = sa_models::QlibImportRequest {
        release_url: std::env::var("QLIB_INIT_RELEASE_URL").ok(),
        dataset_dir: std::env::var("QLIB_INIT_DATASET_DIR").ok(),
        source_path: std::env::var("QLIB_INIT_SOURCE_PATH").ok(),
        force_download: env_truthy("QLIB_INIT_FORCE_DOWNLOAD", false),
        force_reimport: env_truthy("QLIB_INIT_FORCE_REIMPORT", false),
        chunk_size: std::env::var("QLIB_INIT_CHUNK_SIZE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok()),
        symbol_limit: std::env::var("QLIB_INIT_SYMBOL_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok()),
    };
    run_import(data_dir, &request).await
}

#[derive(Debug)]
struct PreparedDataset {
    dataset_root: PathBuf,
    release_url: Option<String>,
    archive_path: Option<String>,
    downloaded: bool,
}

async fn prepare_dataset(
    data_dir: &str,
    request: &sa_models::QlibImportRequest,
) -> anyhow::Result<PreparedDataset> {
    if let Some(source_path) = request
        .source_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(PreparedDataset {
            dataset_root: PathBuf::from(source_path),
            release_url: None,
            archive_path: None,
            downloaded: false,
        });
    }

    let release_url = request
        .release_url
        .clone()
        .unwrap_or_else(|| DEFAULT_QLIB_RELEASE_URL.to_string());
    let dataset_root = request
        .dataset_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(data_dir).join("qlib").join("latest"));
    let archive_dir = PathBuf::from(data_dir).join("qlib").join("archives");
    let archive_path = archive_dir.join("qlib_bin.tar.gz");
    let extracted_root = dataset_root.join("qlib_bin");
    let marker = extracted_root.join("calendars").join("day.txt");

    fs::create_dir_all(&archive_dir)
        .await
        .with_context(|| format!("failed to create {}", archive_dir.display()))?;
    fs::create_dir_all(&dataset_root)
        .await
        .with_context(|| format!("failed to create {}", dataset_root.display()))?;

    let should_download = request.force_download || fs::metadata(&archive_path).await.is_err();
    let mut downloaded = false;
    if should_download {
        let client = reqwest::Client::builder()
            .build()
            .context("failed to build qlib download client")?;
        let response = client
            .get(&release_url)
            .send()
            .await
            .with_context(|| format!("failed to download {release_url}"))?
            .error_for_status()
            .with_context(|| format!("download request failed for {release_url}"))?;
        let bytes = response
            .bytes()
            .await
            .context("failed to read qlib release response body")?;
        let mut file = fs::File::create(&archive_path)
            .await
            .with_context(|| format!("failed to create {}", archive_path.display()))?;
        file.write_all(&bytes)
            .await
            .with_context(|| format!("failed to write {}", archive_path.display()))?;
        downloaded = true;
    }

    let should_extract =
        request.force_reimport || request.force_download || fs::metadata(&marker).await.is_err();
    if should_extract {
        if fs::metadata(&extracted_root).await.is_ok() {
            let _ = fs::remove_dir_all(&extracted_root).await;
        }
        let status = Command::new("tar")
            .arg("-xzf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&dataset_root)
            .status()
            .await
            .with_context(|| format!("failed to spawn tar for {}", archive_path.display()))?;
        if !status.success() {
            anyhow::bail!(
                "failed to extract qlib archive {} into {}",
                archive_path.display(),
                dataset_root.display()
            );
        }
    }

    Ok(PreparedDataset {
        dataset_root,
        release_url: Some(release_url),
        archive_path: Some(archive_path.display().to_string()),
        downloaded,
    })
}

fn detect_qlib_root(path: &Path) -> anyhow::Result<PathBuf> {
    let direct_marker = path.join("calendars").join("day.txt");
    if direct_marker.exists() {
        return Ok(path.to_path_buf());
    }
    let nested = path.join("qlib_bin");
    let nested_marker = nested.join("calendars").join("day.txt");
    if nested_marker.exists() {
        return Ok(nested);
    }
    anyhow::bail!("qlib dataset not found under {}", path.display())
}

async fn load_calendar(qlib_root: &Path) -> anyhow::Result<Vec<String>> {
    let path = qlib_root.join("calendars").join("day.txt");
    let raw = fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    let dates = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if dates.is_empty() {
        anyhow::bail!("calendar {} is empty", path.display());
    }
    Ok(dates)
}

async fn build_symbol_documents(
    symbol_dir: &Path,
    symbol_name: &str,
    calendar: &[String],
    chunk_size: usize,
) -> anyhow::Result<Vec<QlibChunkDocument>> {
    let mut feature_dir = fs::read_dir(symbol_dir)
        .await
        .with_context(|| format!("failed to read {}", symbol_dir.display()))?;
    let mut series_map = BTreeMap::new();
    while let Some(entry) = feature_dir.next_entry().await? {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".day.bin") {
            continue;
        }
        let feature_name = file_name.trim_end_matches(".day.bin").to_string();
        let series = read_feature_series(&path).await?;
        if !series.values.is_empty() {
            series_map.insert(feature_name, series);
        }
    }
    let rows = build_rows(calendar, &series_map)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let ticker = symbol_name.to_uppercase();
    let market = market_for_symbol(symbol_name);
    Ok(rows
        .chunks(chunk_size)
        .filter_map(|chunk| build_chunk_document(&ticker, &market, chunk))
        .collect())
}

async fn read_feature_series(path: &Path) -> anyhow::Result<FeatureSeries> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.len() < 8 || bytes.len() % 4 != 0 {
        anyhow::bail!("unexpected qlib feature size for {}", path.display());
    }
    let values = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect::<Vec<_>>();
    let start_index = values[0].round() as isize;
    if start_index < 0 {
        anyhow::bail!("negative qlib start index for {}", path.display());
    }
    Ok(FeatureSeries {
        start_index: start_index as usize,
        values: values[1..].to_vec(),
    })
}

fn build_rows(
    calendar: &[String],
    series_map: &BTreeMap<String, FeatureSeries>,
) -> anyhow::Result<Vec<QlibRow>> {
    let close = series_map
        .get("close")
        .ok_or_else(|| anyhow::anyhow!("close.day.bin is required for qlib import"))?;
    let mut rows = Vec::new();
    for (offset, close_value) in close.values.iter().enumerate() {
        if !close_value.is_finite() {
            continue;
        }
        let calendar_index = close.start_index + offset;
        let Some(trade_date) = calendar.get(calendar_index) else {
            continue;
        };
        let mut values = BTreeMap::new();
        values.insert("close".to_string(), *close_value);
        for (feature_name, series) in series_map {
            if feature_name == "close" {
                continue;
            }
            if calendar_index < series.start_index {
                continue;
            }
            let series_offset = calendar_index - series.start_index;
            if let Some(value) = series.values.get(series_offset).copied()
                && value.is_finite()
            {
                values.insert(feature_name.clone(), value);
            }
        }
        rows.push(QlibRow {
            trade_date: trade_date.clone(),
            values,
        });
    }
    Ok(rows)
}

fn build_chunk_document(
    ticker: &str,
    market: &str,
    chunk: &[QlibRow],
) -> Option<QlibChunkDocument> {
    let first = chunk.first()?;
    let last = chunk.last()?;
    let first_close = *first.values.get("close")?;
    let last_close = *last.values.get("close")?;
    if !first_close.is_finite() || !last_close.is_finite() || first_close == 0.0 {
        return None;
    }
    let row_count = chunk.len();
    let close_return = (last_close / first_close) - 1.0;
    let high_max = chunk
        .iter()
        .filter_map(|row| row.values.get("high").copied())
        .fold(f32::MIN, f32::max);
    let low_min = chunk
        .iter()
        .filter_map(|row| row.values.get("low").copied())
        .fold(f32::MAX, f32::min);
    let avg_volume = avg_feature(chunk, "volume");
    let avg_amount = avg_feature(chunk, "amount");
    let avg_change = avg_feature(chunk, "change");
    let realized_vol = realized_volatility(chunk);
    let trend_label = if close_return > 0.08 {
        "strong_uptrend"
    } else if close_return > 0.02 {
        "uptrend"
    } else if close_return < -0.08 {
        "strong_downtrend"
    } else if close_return < -0.02 {
        "downtrend"
    } else {
        "rangebound"
    };
    let recent_closes = chunk
        .iter()
        .rev()
        .take(5)
        .filter_map(|row| row.values.get("close"))
        .map(|value| format!("{value:.3}"))
        .collect::<Vec<_>>();
    let recent_changes = chunk
        .iter()
        .rev()
        .take(5)
        .filter_map(|row| row.values.get("change"))
        .map(|value| format!("{value:.4}"))
        .collect::<Vec<_>>();
    let low_min = if low_min == f32::MAX {
        first_close
    } else {
        low_min
    };
    let high_max = if high_max == f32::MIN {
        last_close
    } else {
        high_max
    };

    let text = [
        format!("ticker {ticker}"),
        format!("market {market}"),
        "source qlib_bin".to_string(),
        format!("window {} {}", first.trade_date, last.trade_date),
        format!("row_count {row_count}"),
        format!("trend {trend_label}"),
        format!("close_return {:.4}", close_return),
        format!("realized_volatility {:.4}", realized_vol),
        format!("close_start {:.4}", first_close),
        format!("close_end {:.4}", last_close),
        format!("high_max {:.4}", high_max),
        format!("low_min {:.4}", low_min),
        format!("avg_volume {:.2}", avg_volume),
        format!("avg_amount {:.2}", avg_amount),
        format!("avg_change {:.4}", avg_change),
        format!("recent_closes {}", recent_closes.join(" ")),
        format!("recent_changes {}", recent_changes.join(" ")),
    ]
    .join("\n");

    let payload = json!({
        "ticker": ticker,
        "ticker_lc": ticker.to_ascii_lowercase(),
        "market": market,
        "market_lc": market.to_ascii_lowercase(),
        "entry_kind": "qlib_chunk",
        "source_tag": "qlib_bin",
        "start_date": first.trade_date,
        "end_date": last.trade_date,
        "row_count": row_count,
        "close_start": first_close,
        "close_end": last_close,
        "close_return": close_return,
        "high_max": high_max,
        "low_min": low_min,
        "avg_volume": avg_volume,
        "avg_amount": avg_amount,
        "avg_change": avg_change,
        "realized_volatility": realized_vol,
        "trend": trend_label,
        "text": text,
        "embedding_provider": "hash",
        "embedding_model": "sha256-hash-384"
    });

    Some(QlibChunkDocument {
        ticker: ticker.to_string(),
        market: market.to_string(),
        start_date: first.trade_date.clone(),
        end_date: last.trade_date.clone(),
        text,
        payload,
    })
}

fn avg_feature(rows: &[QlibRow], feature: &str) -> f32 {
    let mut count = 0usize;
    let mut sum = 0.0f32;
    for row in rows {
        if let Some(value) = row.values.get(feature)
            && value.is_finite()
        {
            sum += *value;
            count += 1;
        }
    }
    if count == 0 { 0.0 } else { sum / count as f32 }
}

fn realized_volatility(rows: &[QlibRow]) -> f32 {
    let closes = rows
        .iter()
        .filter_map(|row| row.values.get("close").copied())
        .collect::<Vec<_>>();
    if closes.len() < 2 {
        return 0.0;
    }
    let returns = closes
        .windows(2)
        .filter_map(|window| {
            let prev = window[0];
            let next = window[1];
            (prev != 0.0 && prev.is_finite() && next.is_finite()).then_some((next / prev) - 1.0)
        })
        .collect::<Vec<_>>();
    if returns.is_empty() {
        return 0.0;
    }
    let mean = returns.iter().copied().sum::<f32>() / returns.len() as f32;
    let variance = returns
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f32>()
        / returns.len() as f32;
    variance.sqrt()
}

async fn upsert_documents(
    backend: &QdrantBackend,
    documents: &[QlibChunkDocument],
) -> anyhow::Result<()> {
    for batch in documents.chunks(DEFAULT_BATCH_SIZE) {
        let points = batch
            .iter()
            .map(|document| {
                json!({
                    "id": qdrant_point_id(&format!(
                        "qlib:{}:{}:{}:{}",
                        document.ticker,
                        document.market,
                        document.start_date,
                        document.end_date
                    )),
                    "vector": hash_embed_text(&document.text, DEFAULT_VECTOR_SIZE),
                    "payload": document.payload
                })
            })
            .collect::<Vec<_>>();
        backend
            .http
            .put(format!(
                "{}/collections/{}/points?wait=true",
                backend.url, backend.collection
            ))
            .json(&json!({ "points": points }))
            .send()
            .await
            .context("failed to upsert qlib qdrant points")?
            .error_for_status()
            .context("qdrant qlib upsert request failed")?;
    }
    Ok(())
}

fn build_qdrant_backend() -> anyhow::Result<QdrantBackend> {
    let url = non_empty_env(&["RAG_QDRANT_URL", "QDRANT_URL"])
        .ok_or_else(|| anyhow::anyhow!("RAG_QDRANT_URL or QDRANT_URL is required"))?;
    let collection = non_empty_env(&["RAG_QLIB_QDRANT_COLLECTION"])
        .unwrap_or_else(|| DEFAULT_QLIB_COLLECTION.to_string());
    let api_key = non_empty_env(&["RAG_QDRANT_API_KEY", "QDRANT_API_KEY"]);
    let mut headers = HeaderMap::new();
    if let Some(api_key) = api_key
        && let Ok(api_key_header) = HeaderValue::from_str(&api_key)
    {
        headers.insert("api-key", api_key_header.clone());
        headers.insert(AUTHORIZATION, api_key_header);
    }
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("failed to build qdrant client")?;
    Ok(QdrantBackend {
        http,
        url: url.trim().trim_end_matches('/').to_string(),
        collection,
    })
}

async fn ensure_collection(backend: &QdrantBackend) -> anyhow::Result<()> {
    let response = backend
        .http
        .put(format!(
            "{}/collections/{}",
            backend.url, backend.collection
        ))
        .json(&json!({
            "vectors": {
                "size": DEFAULT_VECTOR_SIZE,
                "distance": "Cosine"
            }
        }))
        .send()
        .await
        .context("failed to ensure qlib qdrant collection")?;
    if !response.status().is_success() && response.status().as_u16() != 409 {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("qlib qdrant ensure collection failed with {status}: {body}");
    }

    for field in [
        "ticker_lc",
        "market_lc",
        "entry_kind",
        "source_tag",
        "start_date",
        "end_date",
        "trend",
    ] {
        let _ = backend
            .http
            .put(format!(
                "{}/collections/{}/index",
                backend.url, backend.collection
            ))
            .json(&json!({
                "field_name": field,
                "field_schema": "keyword"
            }))
            .send()
            .await;
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct QdrantCollectionInfoEnvelope {
    result: QdrantCollectionInfo,
}

#[derive(Debug, serde::Deserialize)]
struct QdrantCollectionInfo {
    #[serde(default)]
    points_count: usize,
}

async fn collection_points(backend: &QdrantBackend) -> anyhow::Result<usize> {
    let response = backend
        .http
        .get(format!(
            "{}/collections/{}",
            backend.url, backend.collection
        ))
        .send()
        .await
        .context("failed to query qlib qdrant collection info")?;
    if response.status().as_u16() == 404 {
        return Ok(0);
    }
    let response = response
        .error_for_status()
        .context("qdrant qlib collection info request failed")?;
    let payload: QdrantCollectionInfoEnvelope = response
        .json()
        .await
        .context("failed to decode qdrant qlib collection info")?;
    Ok(payload.result.points_count)
}

fn hash_embed_text(text: &str, dimension: usize) -> Vec<f32> {
    let mut vector = vec![0.0f32; dimension];
    for token in text
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        let normalized = token.to_ascii_lowercase();
        let digest = Sha256::digest(normalized.as_bytes());
        let index = (u16::from_le_bytes([digest[0], digest[1]]) as usize) % dimension.max(1);
        let sign = if digest[2] % 2 == 0 { 1.0 } else { -1.0 };
        let magnitude = 1.0 + (digest[3] as f32 / 255.0);
        vector[index] += sign * magnitude;
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn qdrant_point_id(entry_id: &str) -> String {
    let digest = Sha256::digest(entry_id.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn non_empty_env(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn env_truthy(key: &str, default: bool) -> bool {
    std::env::var(key)
.map(|v| crate::config::env_flag_value(&v))
        .unwrap_or(default)
}

fn market_for_symbol(symbol: &str) -> String {
    let normalized = symbol.to_ascii_lowercase();
    if normalized.starts_with("sh") || normalized.starts_with("sz") || normalized.starts_with("bj")
    {
        "a_share".to_string()
    } else {
        "unknown".to_string()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_feature_series_supports_qlib_float_header() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("close.day.bin");
        let values = [10f32, 11f32, 12f32, 13f32];
        let mut bytes = Vec::new();
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(&path, bytes).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let series = runtime.block_on(read_feature_series(&path)).unwrap();
        assert_eq!(series.start_index, 10);
        assert_eq!(series.values, vec![11.0, 12.0, 13.0]);
    }

    #[test]
    fn build_rows_and_chunk_document_generates_summary() {
        let calendar = vec![
            "2026-01-01".to_string(),
            "2026-01-02".to_string(),
            "2026-01-05".to_string(),
        ];
        let mut series_map = BTreeMap::new();
        series_map.insert(
            "close".to_string(),
            FeatureSeries {
                start_index: 0,
                values: vec![10.0, 10.5, 11.0],
            },
        );
        series_map.insert(
            "high".to_string(),
            FeatureSeries {
                start_index: 0,
                values: vec![10.1, 10.6, 11.2],
            },
        );
        let rows = build_rows(&calendar, &series_map).unwrap();
        assert_eq!(rows.len(), 3);
        let document = build_chunk_document("SH600000", "a_share", &rows).unwrap();
        assert_eq!(document.start_date, "2026-01-01");
        assert_eq!(document.end_date, "2026-01-05");
        assert!(document.text.contains("trend"));
    }
}

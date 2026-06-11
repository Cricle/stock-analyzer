//! Qlib data import models.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct QlibImportRequest {
    #[serde(default)]
    pub release_url: Option<String>,
    #[serde(default)]
    pub dataset_dir: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub force_download: bool,
    #[serde(default)]
    pub force_reimport: bool,
    #[serde(default)]
    pub chunk_size: Option<usize>,
    #[serde(default)]
    pub symbol_limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QlibImportResponse {
    pub started_at: String,
    pub finished_at: String,
    pub collection: String,
    pub dataset_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_path: Option<String>,
    pub downloaded: bool,
    pub skipped_existing_import: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_points: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
    pub chunk_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_limit: Option<usize>,
    pub imported_symbols: usize,
    pub skipped_symbols: usize,
    pub imported_points: usize,
    pub sample_symbols: Vec<String>,
    pub embedding_provider: String,
}

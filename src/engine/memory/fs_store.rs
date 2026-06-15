//! Filesystem-backed implementation of MemoryStore.

use std::path::PathBuf;

use anyhow::Context;
use async_trait::async_trait;

use super::store::MemoryStore;
use super::{ENTRY_SEPARATOR, MemoryEntry, TradingMemoryLog};

/// Filesystem-backed memory store.
///
/// Stores entries in a single markdown file at `{data_dir}/memory/decisions.md`.
pub struct FilesystemMemoryStore {
    log_path: PathBuf,
}

impl FilesystemMemoryStore {
    pub fn new(data_dir: &str) -> anyhow::Result<Self> {
        let base = PathBuf::from(data_dir).join("memory");
        std::fs::create_dir_all(&base)
            .with_context(|| format!("failed to create {}", base.display()))?;
        Ok(Self {
            log_path: base.join("decisions.md"),
        })
    }
}

#[async_trait]
impl MemoryStore for FilesystemMemoryStore {
    async fn load_entries(&self) -> anyhow::Result<Vec<MemoryEntry>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }
        let text = tokio::fs::read_to_string(&self.log_path)
            .await
            .with_context(|| format!("failed to read {}", self.log_path.display()))?;
        Ok(text
            .split(ENTRY_SEPARATOR)
            .filter_map(TradingMemoryLog::parse_entry)
            .collect())
    }

    async fn append_entry(&self, entry: &str) -> anyhow::Result<()> {
        let mut current = if self.log_path.exists() {
            tokio::fs::read_to_string(&self.log_path)
                .await
                .with_context(|| format!("failed to read {}", self.log_path.display()))?
        } else {
            String::new()
        };
        current.push_str(entry);
        tokio::fs::write(&self.log_path, current)
            .await
            .with_context(|| format!("failed to write {}", self.log_path.display()))?;
        Ok(())
    }

    async fn write_all(&self, content: &str) -> anyhow::Result<()> {
        tokio::fs::write(&self.log_path, content)
            .await
            .with_context(|| format!("failed to write {}", self.log_path.display()))?;
        Ok(())
    }
}

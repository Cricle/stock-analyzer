//! Abstract storage backend for TaskManager.
//!
//! Decouples task artifacts and memory logs from direct filesystem access.

use std::path::PathBuf;
use std::sync::Arc;

/// Abstraction over persistent storage used by TaskManager.
///
/// The default implementation delegates to the local filesystem.
/// Consumers can provide alternative backends (e.g. in-memory for tests,
/// object storage for cloud deployments) by implementing this trait.
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    /// Write `data` to the given path, creating parent directories as needed.
    async fn write_file(&self, path: &str, data: &[u8]) -> anyhow::Result<()>;

    /// Read the entire contents of the file at `path`.
    /// Returns `Ok(None)` if the file does not exist.
    async fn read_file(&self, path: &str) -> anyhow::Result<Option<Vec<u8>>>;

    /// Return `true` if the path exists on disk.
    async fn exists(&self, path: &str) -> bool;

    /// Create the directory and all missing parent directories.
    async fn create_dir_all(&self, path: &str) -> anyhow::Result<()>;
}

/// Filesystem-backed storage rooted at `base_dir`.
#[derive(Clone)]
pub struct FilesystemStorage {
    base_dir: PathBuf,
}

impl FilesystemStorage {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Resolve a relative path against the base directory.
    fn resolve(&self, relative: &str) -> PathBuf {
        self.base_dir.join(relative)
    }
}

#[async_trait::async_trait]
impl StorageBackend for FilesystemStorage {
    async fn write_file(&self, path: &str, data: &[u8]) -> anyhow::Result<()> {
        let full = self.resolve(path);
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&full, data).await?;
        Ok(())
    }

    async fn read_file(&self, path: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let full = self.resolve(path);
        if !full.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read(&full).await?;
        Ok(Some(data))
    }

    async fn exists(&self, path: &str) -> bool {
        self.resolve(path).exists()
    }

    async fn create_dir_all(&self, path: &str) -> anyhow::Result<()> {
        let full = self.resolve(path);
        tokio::fs::create_dir_all(&full).await?;
        Ok(())
    }
}

/// Build a `FilesystemStorage` from the configured data directory.
pub fn default_storage(data_dir: &str) -> Arc<dyn StorageBackend> {
    Arc::new(FilesystemStorage::new(data_dir))
}

//! Trait abstraction for memory storage.

use async_trait::async_trait;

use super::MemoryEntry;

/// Storage backend for trading memory entries.
///
/// Implementations can use filesystem, database, or any other persistence layer.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Load all memory entries from storage.
    async fn load_entries(&self) -> anyhow::Result<Vec<MemoryEntry>>;

    /// Append a raw entry string to the log.
    async fn append_entry(&self, entry: &str) -> anyhow::Result<()>;

    /// Overwrite the entire log with the given content.
    async fn write_all(&self, content: &str) -> anyhow::Result<()>;
}

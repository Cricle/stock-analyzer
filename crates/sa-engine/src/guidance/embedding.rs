//! Semantic text embeddings via fastembed-rs.
//!
//! Uses the AllMiniLML6V2 model (384-dim, supports Chinese + English).
//! The model is lazily loaded on first use and cached for the process lifetime.

#[cfg(feature = "local-rag-embeddings")]
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
#[cfg(feature = "local-rag-embeddings")]
use std::sync::OnceLock;

#[cfg(feature = "local-rag-embeddings")]
static EMBEDDER: OnceLock<Option<TextEmbedding>> = OnceLock::new();

#[cfg(feature = "local-rag-embeddings")]
fn get_embedder() -> Option<&'static TextEmbedding> {
    EMBEDDER
        .get_or_init(|| {
            match TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
            ) {
                Ok(e) => Some(e),
                Err(e) => {
                    tracing::warn!("fastembed model unavailable, falling back to hash embeddings: {e}");
                    None
                }
            }
        })
        .as_ref()
}

/// Generate a 384-dimensional semantic embedding for the given text.
///
/// Uses the AllMiniLML6V2 model via fastembed-rs when available.
/// Falls back to hash-based embedding when the ML model is unavailable.
pub fn semantic_embed(text: &str) -> Vec<f32> {
    #[cfg(feature = "local-rag-embeddings")]
    {
        let Some(embedder) = get_embedder() else {
            return hash_embed(text, EMBEDDING_DIMENSION);
        };
        match embedder.embed(vec![text], None) {
            Ok(mut vectors) => vectors
                .pop()
                .unwrap_or_else(|| vec![0.0f32; EMBEDDING_DIMENSION]),
            Err(e) => {
                tracing::warn!("semantic embedding failed, returning hash fallback: {e}");
                hash_embed(text, EMBEDDING_DIMENSION)
            }
        }
    }
    #[cfg(not(feature = "local-rag-embeddings"))]
    {
        hash_embed(text, EMBEDDING_DIMENSION)
    }
}

/// Embedding dimension for AllMiniLML6V2.
pub const EMBEDDING_DIMENSION: usize = 384;

/// Fallback hash-based embedding when the ML model is unavailable.
///
/// Deterministic hash-based embedding for text (no ML model needed).
/// Kept as a fallback; prefer `semantic_embed` for production use.
pub fn hash_embed(text: &str, dimension: usize) -> Vec<f32> {
    use sha2::{Digest, Sha256};

    let mut vector = vec![0.0f32; dimension];
    for token in text
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
    {
        let normalized = token.to_ascii_lowercase();
        let digest = Sha256::digest(normalized.as_bytes());
        let index = (u16::from_le_bytes([digest[0], digest[1]]) as usize) % dimension.max(1);
        let sign = if digest[2] % 2 == 0 { 1.0 } else { -1.0 };
        let magnitude = 1.0 + (digest[3] as f32 / 255.0);
        vector[index] += sign * magnitude;
    }
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vector {
            *v /= norm;
        }
    }
    vector
}

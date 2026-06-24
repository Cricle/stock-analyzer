use sha2::{Digest, Sha256};
#[cfg(feature = "local-rag-embeddings")]
use std::path::PathBuf;

use super::{EmbeddingBackend, RagConfig, TradingMemoryLog, VectorMemoryBackend};

impl TradingMemoryLog {
    pub(super) fn load_rag_config() -> RagConfig {
        let snapshot = Self::rag_runtime_snapshot();
        RagConfig {
            enabled: snapshot.enabled,
            embedding_provider: snapshot.embedding_provider,
            embedding_model: snapshot.embedding_model,
            top_k: std::env::var("RAG_TOP_K")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(8)
                .max(1),
            same_ticker_top_k: std::env::var("RAG_SAME_TICKER_TOP_K")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(5)
                .max(1),
            cross_ticker_top_k: std::env::var("RAG_CROSS_TICKER_TOP_K")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(3)
                .max(1),
        }
    }

    /// Build a vector store backend from environment variables.
    ///
    /// TODO: This previously built a QdrantClient directly. With the trait-based
    /// approach, callers should inject a concrete VectorStore implementation.
    /// Returns None when RAG is disabled.
    pub(super) fn build_vector_backend(_rag: &RagConfig) -> Option<VectorMemoryBackend> {
        // In the trait-based architecture, the vector store is injected externally.
        // Return None here; callers should use TradingMemoryLog::with_vector_store() instead.
        None
    }

    pub(super) fn build_embedding_backend(_data_dir: &str, rag: &RagConfig) -> EmbeddingBackend {
        if !rag.enabled {
            return EmbeddingBackend {
                provider: "disabled".to_string(),
                model: rag.embedding_model.clone(),
                dimension: 384,
                retrieval_enabled: false,
                failure_reason: None,
                #[cfg(feature = "local-rag-embeddings")]
                inner: None,
            };
        }

        match rag.embedding_provider.trim().to_ascii_lowercase().as_str() {
            "fastembed" | "local" => {
                #[cfg(feature = "local-rag-embeddings")]
                {
                    use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
                    use std::sync::Arc;

                    let mut options = InitOptions::default();
                    options.model_name = match rag.embedding_model.as_str() {
                        "BAAI/bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
                        "BAAI/bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
                        "BAAI/bge-large-en-v1.5" => EmbeddingModel::BGELargeENV15,
                        _ => EmbeddingModel::BGESmallENV15,
                    };
                    options.cache_dir = PathBuf::from(_data_dir).join("models").join("fastembed");
                    options.show_download_progress = false;
                    let (inner, dimension, failure_reason) = match TextEmbedding::try_new(options) {
                        Ok(model) => {
                            let model = Arc::new(model);
                            let dimension = model
                                .embed(vec!["probe".to_string()], None)
                                .ok()
                                .and_then(|vectors| vectors.into_iter().next())
                                .map(|vector| vector.len())
                                .unwrap_or(384);
                            (Some(model), dimension, None)
                        }
                        Err(error) => {
                            let reason = format!("fastembed init failed: {error}");
                            tracing::warn!(reason = %reason, "rag embedding initialization failed");
                            (None, 384, Some(reason))
                        }
                    };
                    let retrieval_enabled = inner.is_some();
                    let provider = if retrieval_enabled {
                        "fastembed".to_string()
                    } else {
                        "fastembed-unavailable".to_string()
                    };
                    return EmbeddingBackend {
                        inner,
                        provider,
                        model: rag.embedding_model.clone(),
                        dimension,
                        retrieval_enabled,
                        failure_reason,
                    };
                }

                #[cfg(not(feature = "local-rag-embeddings"))]
                EmbeddingBackend {
                    provider: "fastembed-unavailable".to_string(),
                    model: rag.embedding_model.clone(),
                    dimension: 384,
                    retrieval_enabled: false,
                    failure_reason: Some(
                        "binary built without local-rag-embeddings feature".to_string(),
                    ),
                }
            }
            "hash" | "hash-fallback" => EmbeddingBackend {
                provider: "hash".to_string(),
                model: rag.embedding_model.clone(),
                dimension: 384,
                retrieval_enabled: true,
                failure_reason: None,
                #[cfg(feature = "local-rag-embeddings")]
                inner: None,
            },
            other => EmbeddingBackend {
                provider: other.to_string(),
                model: rag.embedding_model.clone(),
                dimension: 384,
                retrieval_enabled: false,
                failure_reason: Some(format!("unsupported embedding provider: {other}")),
                #[cfg(feature = "local-rag-embeddings")]
                inner: None,
            },
        }
    }

    pub(super) fn hash_embed_text(text: &str, dimension: usize) -> Vec<f32> {
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

    pub fn embed_text(&self, text: &str) -> Vec<f32> {
        #[cfg(feature = "local-rag-embeddings")]
        {
            if let Some(model) = &self.embedding.inner {
                if let Ok(mut vectors) = model.embed(vec![text.to_string()], None) {
                    if let Some(vector) = vectors.pop() {
                        return vector;
                    }
                }
            }
        }

        // Always fall back to hash embedding to guarantee a non-empty vector
        Self::hash_embed_text(text, self.embedding.dimension)
    }
}

use sa::guide::embedding::EMBEDDING_DIMENSION;
use sa::guide::{hash_embed, semantic_embed};

#[test]
fn hash_embed_returns_correct_dimension() {
    let vec = hash_embed("hello world", 384);
    assert_eq!(vec.len(), 384);
}

#[test]
fn hash_embed_custom_dimension() {
    let vec = hash_embed("test", 64);
    assert_eq!(vec.len(), 64);
}

#[test]
fn hash_embed_is_normalized() {
    let vec = hash_embed("hello world", 128);
    let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.01,
        "expected unit norm, got {}",
        norm
    );
}

#[test]
fn hash_embed_empty_string_is_zero_vector() {
    let vec = hash_embed("", 128);
    let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!(norm < f32::EPSILON, "expected zero vector for empty input");
}

#[test]
fn hash_embed_deterministic() {
    let a = hash_embed("deterministic test", 256);
    let b = hash_embed("deterministic test", 256);
    assert_eq!(a, b);
}

#[test]
fn hash_embed_different_inputs_differ() {
    let a = hash_embed("hello", 128);
    let b = hash_embed("world", 128);
    assert_ne!(a, b);
}

#[test]
fn hash_embed_single_token() {
    let vec = hash_embed("token", 64);
    let non_zero = vec.iter().filter(|v| v.abs() > f32::EPSILON).count();
    assert!(non_zero > 0, "expected at least one non-zero element");
}

#[test]
fn hash_embed_multi_token_accumulates() {
    let single = hash_embed("hello", 256);
    let multi = hash_embed("hello world foo bar", 256);
    let single_nonzero = single.iter().filter(|v| v.abs() > f32::EPSILON).count();
    let multi_nonzero = multi.iter().filter(|v| v.abs() > f32::EPSILON).count();
    assert!(multi_nonzero >= single_nonzero);
}

#[test]
fn semantic_embed_returns_correct_dimension() {
    let vec = semantic_embed("test text");
    assert_eq!(vec.len(), EMBEDDING_DIMENSION);
}

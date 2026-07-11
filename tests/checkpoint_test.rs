use stock_analyzer::checkpoint::{TaskCheckpointStore, hex_16};

#[test]
fn thread_id_deterministic() {
    let a = TaskCheckpointStore::thread_id("task-1", "AAPL", "2025-01-15");
    let b = TaskCheckpointStore::thread_id("task-1", "AAPL", "2025-01-15");
    assert_eq!(a, b);
}

#[test]
fn thread_id_different_task_id() {
    let a = TaskCheckpointStore::thread_id("task-1", "AAPL", "2025-01-15");
    let b = TaskCheckpointStore::thread_id("task-2", "AAPL", "2025-01-15");
    assert_ne!(a, b);
}

#[test]
fn thread_id_different_symbol() {
    let a = TaskCheckpointStore::thread_id("task-1", "AAPL", "2025-01-15");
    let b = TaskCheckpointStore::thread_id("task-1", "MSFT", "2025-01-15");
    assert_ne!(a, b);
}

#[test]
fn thread_id_different_date() {
    let a = TaskCheckpointStore::thread_id("task-1", "AAPL", "2025-01-15");
    let b = TaskCheckpointStore::thread_id("task-1", "AAPL", "2025-01-16");
    assert_ne!(a, b);
}

#[test]
fn thread_id_case_insensitive_symbol() {
    let a = TaskCheckpointStore::thread_id("task-1", "aapl", "2025-01-15");
    let b = TaskCheckpointStore::thread_id("task-1", "AAPL", "2025-01-15");
    assert_eq!(a, b);
}

#[test]
fn thread_id_is_16_chars() {
    let id = TaskCheckpointStore::thread_id("task-1", "AAPL", "2025-01-15");
    assert_eq!(id.len(), 16);
}

#[test]
fn thread_id_hex_chars_only() {
    let id = TaskCheckpointStore::thread_id("task-1", "AAPL", "2025-01-15");
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn thread_id_empty_inputs() {
    let id = TaskCheckpointStore::thread_id("", "", "");
    assert_eq!(id.len(), 16);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn hex_16_basic() {
    let bytes = [0x01u8, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xff];
    let hex = hex_16(&bytes);
    assert_eq!(hex, "0123456789abcdef");
}

#[test]
fn hex_16_short_input() {
    let bytes = [0xdeu8, 0xad];
    let hex = hex_16(&bytes);
    assert_eq!(hex, "dead");
}

#[test]
fn hex_16_empty_input() {
    let bytes: [u8; 0] = [];
    let hex = hex_16(&bytes);
    assert!(hex.is_empty());
}

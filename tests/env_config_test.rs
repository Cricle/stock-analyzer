use sa::env_config::{env_flag, env_flag_value};

#[test]
fn env_flag_value_true_variants() {
    for v in &["1", "true", "TRUE", "yes", "YES"] {
        assert!(env_flag_value(v), "expected true for {:?}", v);
    }
}

#[test]
fn env_flag_value_false_variants() {
    for v in &["0", "false", "FALSE", "no", "NO", "", "True", "Yes", "y"] {
        assert!(!env_flag_value(v), "expected false for {:?}", v);
    }
}

#[test]
fn env_flag_missing_returns_false() {
    assert!(!env_flag("DEFINITELY_NOT_SET_VAR_12345"));
}

use sa::analysis::priority_rank;

#[test]
fn priority_rank_high() {
    assert_eq!(priority_rank("high"), 0);
}

#[test]
fn priority_rank_medium() {
    assert_eq!(priority_rank("medium"), 1);
}

#[test]
fn priority_rank_low() {
    assert_eq!(priority_rank("low"), 2);
}

#[test]
fn priority_rank_unknown() {
    assert_eq!(priority_rank("unknown"), 2);
}

#[test]
fn priority_rank_empty() {
    assert_eq!(priority_rank(""), 2);
}

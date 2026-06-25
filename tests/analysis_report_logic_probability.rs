use sa::analysis::round_to_100;

#[test]
fn round_to_100_sums_exactly() {
    // Try many combinations to ensure the sum is always exactly 100.
    let cases = [
        (33.333, 33.333, 33.334),
        (50.0, 30.0, 20.0),
        (10.0, 10.0, 80.0),
        (5.0, 5.0, 90.0),
        (45.6, 27.3, 27.1),
        (34.0, 32.0, 34.0),
    ];
    for (u, d, s) in cases {
        let (ru, rd, rs) = round_to_100(u, d, s);
        assert_eq!(
            ru + rd + rs,
            100.0,
            "round_to_100({}, {}, {}) = ({}, {}, {}) sums to {}",
            u,
            d,
            s,
            ru,
            rd,
            rs,
            ru + rd + rs
        );
    }
}

#[test]
fn round_to_100_adjusts_largest() {
    // The residual should be added to the largest component.
    let (u, d, s) = round_to_100(60.4, 20.3, 19.3);
    assert_eq!(u + d + s, 100.0);
    // 60.4 rounds to 60, 20.3 rounds to 20, 19.3 rounds to 19 => 99
    // Largest is 60, so it gets +1 => 61
    assert_eq!(u, 61.0);
}

#[test]
fn round_to_100_already_exact() {
    let (u, d, s) = round_to_100(50.0, 30.0, 20.0);
    assert_eq!((u, d, s), (50.0, 30.0, 20.0));
    assert_eq!(u + d + s, 100.0);
}

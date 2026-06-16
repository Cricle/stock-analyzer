
    #[test]
    fn publishable_summary_reference_accepts_non_empty() {
        assert!(super::is_publishable_summary_reference("确认后再评估上行空间"));
        assert!(super::is_publishable_summary_reference("311.4上方有效突破"));
        assert!(super::is_publishable_summary_reference("跌破270.55"));
        assert!(!super::is_publishable_summary_reference("a"));
        assert!(!super::is_publishable_summary_reference(""));
    }


fn bool_text(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}

fn count_numeric_levels(text: &str) -> i32 {
    numeric_tokens(text)
        .into_iter()
        .filter(|token| {
            let integer_digits = token
                .split_once('.')
                .map(|(left, _)| left)
                .unwrap_or(token.as_str())
                .trim_start_matches('-')
                .len();
            (2..=5).contains(&integer_digits)
        })
        .count() as i32
}

fn count_numeric_dates(text: &str) -> i32 {
    text.split_whitespace()
        .filter(|token| {
            looks_like_ymd_date(
                token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '-' && ch != '/'),
            )
        })
        .count() as i32
}

fn parse_first_number(text: &str) -> Option<f64> {
    numeric_tokens(text)
        .into_iter()
        .find_map(|token| token.parse::<f64>().ok())
}

fn parse_position_percentage(text: &str) -> Option<f64> {
    let value = parse_first_number(text)?;
    if text.chars().any(|ch| ch == '%') {
        Some((value / 100.0).clamp(0.0, 1.0))
    } else if (0.0..=1.0).contains(&value) {
        Some(value)
    } else if (1.0..=100.0).contains(&value) {
        Some(value / 100.0)
    } else {
        None
    }
}

fn numeric_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        let allowed = ch.is_ascii_digit()
            || (ch == '.' && current.chars().any(|inner| inner.is_ascii_digit()))
            || (ch == '-' && current.is_empty());
        if allowed {
            current.push(ch);
        } else if current.chars().any(|inner| inner.is_ascii_digit()) {
            tokens.push(current.clone());
            current.clear();
        } else {
            current.clear();
        }
    }
    if current.chars().any(|inner| inner.is_ascii_digit()) {
        tokens.push(current);
    }
    tokens
}

fn looks_like_ymd_date(token: &str) -> bool {
    let separator = if token.contains('-') {
        '-'
    } else if token.contains('/') {
        '/'
    } else {
        return false;
    };
    let parts = token.split(separator).collect::<Vec<_>>();
    if parts.len() != 3 {
        return false;
    }
    let year = parts[0];
    let month = parts[1];
    let day = parts[2];
    year.len() == 4
        && (1..=2).contains(&month.len())
        && (1..=2).contains(&day.len())
        && year.chars().all(|ch| ch.is_ascii_digit())
        && month.chars().all(|ch| ch.is_ascii_digit())
        && day.chars().all(|ch| ch.is_ascii_digit())
}

trait NumericFieldExt {
    fn numeric_count(&self) -> i32;
}

impl NumericFieldExt for str {
    fn numeric_count(&self) -> i32 {
        numeric_tokens(self).len() as i32
    }
}

#[cfg(test)]
mod fundamental_tests {
    use super::*;

    // --- numeric_tokens ---

    #[test]
    fn numeric_tokens_simple() {
        let tokens = numeric_tokens("price is 123.45");
        assert_eq!(tokens, vec!["123.45"]);
    }

    #[test]
    fn numeric_tokens_negative() {
        let tokens = numeric_tokens("drop of -5.2%");
        assert_eq!(tokens, vec!["-5.2"]);
    }

    #[test]
    fn numeric_tokens_multiple() {
        let tokens = numeric_tokens("entry 100 stop 95 target 110");
        assert_eq!(tokens, vec!["100", "95", "110"]);
    }

    #[test]
    fn numeric_tokens_empty() {
        let tokens = numeric_tokens("no numbers here");
        assert!(tokens.is_empty());
    }

    #[test]
    fn numeric_tokens_only_dot() {
        let tokens = numeric_tokens("just a dot . here");
        assert!(tokens.is_empty());
    }

    #[test]
    fn numeric_tokens_leading_dot_not_supported() {
        // Parser requires a digit before the dot
        let tokens = numeric_tokens("value .5 percent");
        assert_eq!(tokens, vec!["5"]);
    }

    // --- count_numeric_levels ---

    #[test]
    fn count_numeric_levels_2_to_5_digits() {
        assert_eq!(count_numeric_levels("price at 1234"), 1);
        assert_eq!(count_numeric_levels("12 and 12345"), 2);
    }

    #[test]
    fn count_numeric_levels_too_short_or_long() {
        assert_eq!(count_numeric_levels("1 and 123456"), 0);
    }

    #[test]
    fn count_numeric_levels_mixed() {
        assert_eq!(count_numeric_levels("entry 100.50 stop 95"), 2);
    }

    #[test]
    fn count_numeric_levels_empty() {
        assert_eq!(count_numeric_levels(""), 0);
    }

    // --- count_numeric_dates ---

    #[test]
    fn count_numeric_dates_ymd() {
        assert_eq!(count_numeric_dates("report from 2026-06-21"), 1);
        assert_eq!(count_numeric_dates("2026-01-01 to 2026-12-31"), 2);
    }

    #[test]
    fn count_numeric_dates_slash() {
        assert_eq!(count_numeric_dates("date 2026/06/21"), 1);
    }

    #[test]
    fn count_numeric_dates_none() {
        assert_eq!(count_numeric_dates("no dates here"), 0);
    }

    #[test]
    fn count_numeric_dates_short_year() {
        assert_eq!(count_numeric_dates("26-06-21"), 0);
    }

    // --- parse_first_number ---

    #[test]
    fn parse_first_number_basic() {
        assert_eq!(parse_first_number("price is 123.45"), Some(123.45));
    }

    #[test]
    fn parse_first_number_negative() {
        assert_eq!(parse_first_number("drop -5.2"), Some(-5.2));
    }

    #[test]
    fn parse_first_number_none() {
        assert_eq!(parse_first_number("no numbers"), None);
    }

    #[test]
    fn parse_first_number_first_wins() {
        assert_eq!(parse_first_number("100 and 200"), Some(100.0));
    }

    // --- parse_position_percentage ---

    #[test]
    fn parse_position_percentage_with_percent() {
        assert_eq!(parse_position_percentage("20%"), Some(0.2));
    }

    #[test]
    fn parse_position_percentage_decimal() {
        assert_eq!(parse_position_percentage("0.2"), Some(0.2));
    }

    #[test]
    fn parse_position_percentage_whole_number() {
        assert_eq!(parse_position_percentage("20"), Some(0.2));
    }

    #[test]
    fn parse_position_percentage_out_of_range() {
        assert_eq!(parse_position_percentage("150"), None);
    }

    #[test]
    fn parse_position_percentage_empty() {
        assert_eq!(parse_position_percentage(""), None);
    }

    // --- looks_like_ymd_date ---

    #[test]
    fn looks_like_ymd_valid_dash() {
        assert!(looks_like_ymd_date("2026-06-21"));
    }

    #[test]
    fn looks_like_ymd_valid_slash() {
        assert!(looks_like_ymd_date("2026/6/1"));
    }

    #[test]
    fn looks_like_ymd_invalid_short_year() {
        assert!(!looks_like_ymd_date("26-06-21"));
    }

    #[test]
    fn looks_like_ymd_invalid_two_parts() {
        assert!(!looks_like_ymd_date("2026-06"));
    }

    #[test]
    fn looks_like_ymd_invalid_no_separator() {
        assert!(!looks_like_ymd_date("hello"));
    }

    // --- bool_text ---

    #[test]
    fn bool_text_true_false() {
        assert_eq!(bool_text(true), "是");
        assert_eq!(bool_text(false), "否");
    }

    // --- NumericFieldExt ---

    #[test]
    fn numeric_field_ext_count() {
        assert_eq!("entry 100 stop 95".numeric_count(), 2);
        assert_eq!("no numbers".numeric_count(), 0);
    }
}

pub fn bool_text(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}

pub fn count_numeric_levels(text: &str) -> i32 {
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

pub fn count_numeric_dates(text: &str) -> i32 {
    text.split_whitespace()
        .filter(|token| {
            looks_like_ymd_date(
                token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '-' && ch != '/'),
            )
        })
        .count() as i32
}

pub fn parse_first_number(text: &str) -> Option<f64> {
    numeric_tokens(text)
        .into_iter()
        .find_map(|token| token.parse::<f64>().ok())
}

pub fn parse_position_percentage(text: &str) -> Option<f64> {
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

pub fn numeric_tokens(text: &str) -> Vec<String> {
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

pub fn looks_like_ymd_date(token: &str) -> bool {
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

//! Internationalization support with bundled locale JSON files.

use std::collections::HashMap;

use serde_json::Value;

/// Loads bundled zh/en locale files at compile time and resolves
/// dot-separated translation keys with optional `{param}` interpolation.
pub struct I18n {
    locales: HashMap<String, Value>,
}

impl I18n {
    /// Build a new `I18n` instance with the bundled zh and en locales.
    pub fn new() -> Self {
        let zh: Value =
            serde_json::from_str(include_str!("locales/zh.json")).expect("invalid zh.json");
        let en: Value =
            serde_json::from_str(include_str!("locales/en.json")).expect("invalid en.json");

        let mut locales = HashMap::new();
        locales.insert("zh".to_string(), zh);
        locales.insert("en".to_string(), en);
        Self { locales }
    }

    /// Look up a dot-separated key (e.g. `"report.summary.title"`) in the
    /// given language. Returns `None` if the language or key path is missing.
    pub fn resolve(&self, key: &str, lang: &str) -> Option<String> {
        let locale = self.locales.get(lang)?;
        let mut current = locale;
        for segment in key.split('.') {
            match current {
                Value::Object(map) => {
                    current = map.get(segment)?;
                }
                _ => return None,
            }
        }
        current.as_str().map(String::from)
    }

    /// Like [`resolve`](Self::resolve), but replaces `{param_name}` and
    /// `{param_name:format}` placeholders in the resolved string with values
    /// from `params`.  Format specifiers like `:.1` are applied to numeric values.
    pub fn resolve_with_params(
        &self,
        key: &str,
        lang: &str,
        params: &serde_json::Map<String, Value>,
    ) -> Option<String> {
        let template = self.resolve(key, lang)?;
        let re = regex::Regex::new(r"\{(\w+)(?::([^}]+))?\}").ok()?;
        let result = re.replace_all(&template, |caps: &regex::Captures| {
            let name = &caps[1];
            let fmt = caps.get(2).map(|m| m.as_str());
            match params.get(name) {
                Some(value) => format_value(value, fmt),
                None => caps[0].to_string(),
            }
        });
        Some(result.into_owned())
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a JSON value with an optional Rust-style format specifier.
/// Supports: `:.N` (N decimal places), `+.N` (signed, N decimal places), `:N` (zero-padded integer).
fn format_value(value: &Value, fmt: Option<&str>) -> String {
    match (value, fmt) {
        (Value::Number(n), Some(spec)) => {
            let f = match n.as_f64() {
                Some(f) => f,
                None => return n.to_string(),
            };
            // Parse optional `+` flag and precision.
            let (sign, rest) = if let Some(r) = spec.strip_prefix('+') {
                (true, r)
            } else {
                (false, spec)
            };
            if let Some(precision) = rest.strip_prefix('.')
                && let Ok(p) = precision.trim_end_matches('f').parse::<usize>()
            {
                if sign {
                    return format!("{:+.p$}", f, p = p);
                }
                return format!("{:.p$}", f, p = p);
            }
            if sign {
                return format!("{:+}", f);
            }
            n.to_string()
        }
        (Value::String(s), _) => s.clone(),
        (Value::Number(n), None) => {
            if let Some(f) = n.as_f64()
                && f == f.floor()
                && f.abs() < 1e15
            {
                return format!("{}", f as i64);
            }
            n.to_string()
        }
        (other, _) => other.to_string(),
    }
}

#[cfg(test)]
mod tests;

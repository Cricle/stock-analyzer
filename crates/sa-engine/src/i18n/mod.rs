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

    /// Like [`resolve`](Self::resolve), but replaces `{param_name}` placeholders
    /// in the resolved string with values from `params`.
    pub fn resolve_with_params(
        &self,
        key: &str,
        lang: &str,
        params: &serde_json::Map<String, Value>,
    ) -> Option<String> {
        let template = self.resolve(key, lang)?;
        let mut result = template;
        for (name, value) in params {
            let placeholder = format!("{{{name}}}");
            let replacement = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }
        Some(result)
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_zh_key() {
        let i18n = I18n::new();
        let result = i18n.resolve("report.summary.title", "zh");
        assert_eq!(result.as_deref(), Some("分析摘要"));
    }

    #[test]
    fn resolve_en_key() {
        let i18n = I18n::new();
        let result = i18n.resolve("report.summary.title", "en");
        assert_eq!(result.as_deref(), Some("Analysis Summary"));
    }

    #[test]
    fn resolve_missing_key() {
        let i18n = I18n::new();
        let result = i18n.resolve("nonexistent.key", "zh");
        assert_eq!(result, None);
    }

    #[test]
    fn resolve_with_params() {
        let i18n = I18n::new();
        let mut params = serde_json::Map::new();
        params.insert("symbol".to_string(), Value::String("AAPL".to_string()));
        let result = i18n.resolve_with_params("report.header.for_symbol", "zh", &params);
        assert_eq!(result.as_deref(), Some("AAPL 的分析报告"));
    }
}

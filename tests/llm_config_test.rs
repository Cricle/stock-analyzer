use stock_analyzer::llm_config::LlmProviderConfig;

#[test]
fn llm_provider_config_serde_roundtrip() {
    let config = LlmProviderConfig {
        id: "test-id".to_string(),
        display_name: "Test Provider".to_string(),
        base_url: "https://api.example.com".to_string(),
        api_key: Some("sk-test".to_string()),
        default_model: "model-v1".to_string(),
        quick_model: Some("model-quick".to_string()),
        deep_model: Some("model-deep".to_string()),
        quick_input_price_per_million: Some(1.5),
        quick_output_price_per_million: Some(2.0),
        deep_input_price_per_million: Some(3.0),
        deep_output_price_per_million: Some(4.0),
        enabled: true,
        is_default: false,
        provider_type: Some("anthropic".to_string()),
        created_at: "2025-01-01".to_string(),
        updated_at: "2025-01-02".to_string(),
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: LlmProviderConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "test-id");
    assert_eq!(deserialized.display_name, "Test Provider");
    assert_eq!(deserialized.base_url, "https://api.example.com");
    assert_eq!(deserialized.api_key, Some("sk-test".to_string()));
    assert_eq!(deserialized.default_model, "model-v1");
    assert!(deserialized.enabled);
    assert!(!deserialized.is_default);
}

#[test]
fn llm_provider_config_minimal() {
    let config = LlmProviderConfig {
        id: "min".to_string(),
        display_name: "Min".to_string(),
        base_url: "http://localhost".to_string(),
        api_key: None,
        default_model: "m".to_string(),
        quick_model: None,
        deep_model: None,
        quick_input_price_per_million: None,
        quick_output_price_per_million: None,
        deep_input_price_per_million: None,
        deep_output_price_per_million: None,
        enabled: false,
        is_default: true,
        provider_type: None,
        created_at: "".to_string(),
        updated_at: "".to_string(),
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: LlmProviderConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "min");
    assert!(deserialized.api_key.is_none());
    assert!(!deserialized.enabled);
    assert!(deserialized.is_default);
}

#[test]
fn llm_provider_config_from_json() {
    let json = r#"{
        "id": "from-json",
        "display_name": "From JSON",
        "base_url": "http://test",
        "default_model": "m",
        "enabled": true,
        "is_default": false,
        "created_at": "now",
        "updated_at": "now"
    }"#;
    let config: LlmProviderConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.id, "from-json");
    assert!(config.api_key.is_none());
    assert!(config.quick_model.is_none());
}

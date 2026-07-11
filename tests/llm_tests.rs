use stock_analyzer::llm::{LlmClient, LlmUsageAccumulator};

fn make_client() -> LlmClient {
    let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    LlmClient::openai_compatible(http, "https://api.example.com/v1/", "sk-test", "gpt-4", 30)
}

#[test]
fn with_model_replaces_model() {
    let client = make_client();
    let updated = client.with_model(Some("claude-3"));
    assert_eq!(updated.model, "claude-3");
    assert_eq!(updated.openai_base_url, "https://api.example.com/v1");
}

#[test]
fn with_model_none_keeps_original() {
    let client = make_client();
    let updated = client.with_model(None);
    assert_eq!(updated.model, "gpt-4");
}

#[test]
fn with_model_empty_string_keeps_original() {
    let client = make_client();
    let updated = client.with_model(Some("  "));
    assert_eq!(updated.model, "gpt-4");
}

#[test]
fn with_base_url_replaces_url() {
    let client = make_client();
    let updated = client.with_base_url(Some("https://other.com/api"));
    assert_eq!(updated.openai_base_url, "https://other.com/api");
}

#[test]
fn with_base_url_strips_trailing_slash() {
    let client = make_client();
    let updated = client.with_base_url(Some("https://other.com/api/"));
    assert_eq!(updated.openai_base_url, "https://other.com/api");
}

#[test]
fn with_base_url_none_keeps_original() {
    let client = make_client();
    let updated = client.with_base_url(None);
    assert_eq!(updated.openai_base_url, "https://api.example.com/v1");
}

#[test]
fn with_api_key_replaces_key() {
    let client = make_client();
    let updated = client.with_api_key(Some("sk-new-key"));
    assert_eq!(updated.openai_api_key, "sk-new-key");
}

#[test]
fn with_api_key_none_keeps_original() {
    let client = make_client();
    let updated = client.with_api_key(None);
    assert_eq!(updated.openai_api_key, "sk-test");
}

#[test]
fn with_api_key_empty_keeps_original() {
    let client = make_client();
    let updated = client.with_api_key(Some("  "));
    assert_eq!(updated.openai_api_key, "sk-test");
}

#[test]
fn openai_compatible_strips_trailing_slash() {
    let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let client =
        LlmClient::openai_compatible(http, "https://api.example.com/v1/", "key", "model", 60);
    assert_eq!(client.openai_base_url, "https://api.example.com/v1");
    assert_eq!(client.provider_type, "openai");
}

#[test]
fn anthropic_sets_provider_type() {
    let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
    let client = LlmClient::anthropic(http, "https://api.anthropic.com/", "key", "claude-3", 60);
    assert_eq!(client.provider_type, "anthropic");
    assert_eq!(client.openai_base_url, "https://api.anthropic.com");
}

#[test]
fn usage_accumulator_default() {
    let acc = LlmUsageAccumulator::default();
    assert_eq!(acc.total_requests, 0);
    assert_eq!(acc.prompt_tokens, 0);
    assert_eq!(acc.completion_tokens, 0);
    assert_eq!(acc.total_tokens, 0);
    assert!(acc.by_model.is_empty());
}

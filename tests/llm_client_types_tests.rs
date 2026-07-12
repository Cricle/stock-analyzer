use serde_json::json;
use stock_analyzer::llm::client::{
    AnthropicResponse, ChatCompletionRequest, ChatCompletionResponse, ChatMessage,
    ChatMessageResponse, approximate_tokens_from_chars, estimate_chat_completion_usage,
};

#[test]
fn approximate_tokens_from_chars_short_text() {
    assert_eq!(approximate_tokens_from_chars(1), 1);
}

#[test]
fn approximate_tokens_from_chars_100_chars() {
    assert_eq!(approximate_tokens_from_chars(100), 25);
}

#[test]
fn approximate_tokens_from_chars_zero() {
    assert_eq!(approximate_tokens_from_chars(0), 1);
}

#[test]
fn estimate_chat_completion_usage_basic() {
    let request = ChatCompletionRequest {
        model: "gpt-4".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "Hello world".into(),
        }],
        temperature: 0.7,
        response_format: None,
        tools: None,
        tool_choice: None,
    };
    let usage = estimate_chat_completion_usage(&request, "Hi there");
    assert!(usage.prompt_tokens > 0);
    assert!(usage.completion_tokens > 0);
    assert_eq!(
        usage.total_tokens,
        usage.prompt_tokens + usage.completion_tokens
    );
}

#[test]
fn content_text_from_string_content() {
    let response = ChatMessageResponse {
        content: Some(json!("hello world")),
        refusal: None,
        tool_calls: None,
    };
    assert_eq!(response.content_text(), "hello world");
}

#[test]
fn content_text_from_array_with_text_field() {
    let response = ChatMessageResponse {
        content: Some(json!([{ "type": "text", "text": "result" }])),
        refusal: None,
        tool_calls: None,
    };
    assert_eq!(response.content_text(), "result");
}

#[test]
fn content_text_from_array_with_content_field() {
    let response = ChatMessageResponse {
        content: Some(json!([{ "content": "data" }])),
        refusal: None,
        tool_calls: None,
    };
    assert_eq!(response.content_text(), "data");
}

#[test]
fn content_text_falls_back_to_refusal_string() {
    let response = ChatMessageResponse {
        content: None,
        refusal: Some(json!("refused content")),
        tool_calls: None,
    };
    assert_eq!(response.content_text(), "refused content");
}

#[test]
fn content_text_falls_back_to_refusal_array() {
    let response = ChatMessageResponse {
        content: None,
        refusal: Some(json!([{ "text": "refused" }])),
        tool_calls: None,
    };
    assert_eq!(response.content_text(), "refused");
}

#[test]
fn content_text_falls_back_to_tool_calls() {
    let response = ChatMessageResponse {
        content: None,
        refusal: None,
        tool_calls: Some(vec![json!({"id": "c1"})]),
    };
    let text = response.content_text();
    assert!(text.contains("c1"));
}

#[test]
fn content_text_empty_when_all_none() {
    let response = ChatMessageResponse {
        content: None,
        refusal: None,
        tool_calls: None,
    };
    assert_eq!(response.content_text(), "");
}

#[test]
fn content_text_null_content_returns_empty() {
    let response = ChatMessageResponse {
        content: Some(json!(null)),
        refusal: None,
        tool_calls: None,
    };
    assert_eq!(response.content_text(), "");
}

#[test]
fn content_text_number_content() {
    let response = ChatMessageResponse {
        content: Some(json!(42)),
        refusal: None,
        tool_calls: None,
    };
    assert_eq!(response.content_text(), "42");
}

#[test]
fn chat_completion_response_deserializes() {
    let json_str = r#"{"choices":[{"message":{"content":"test"}}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
    let resp: ChatCompletionResponse = serde_json::from_str(json_str).unwrap();
    assert_eq!(resp.choices.len(), 1);
    assert!(resp.usage.is_some());
    assert_eq!(resp.usage.unwrap().total_tokens, 15);
}

#[test]
fn anthropic_response_deserializes() {
    let json_str =
        r#"{"content":[{"text":"hello"}],"usage":{"input_tokens":10,"output_tokens":5}}"#;
    let resp: AnthropicResponse = serde_json::from_str(json_str).unwrap();
    assert_eq!(resp.content.len(), 1);
    assert_eq!(resp.content[0].text.as_deref(), Some("hello"));
    assert_eq!(resp.usage.input_tokens, Some(10));
    assert_eq!(resp.usage.output_tokens, Some(5));
}

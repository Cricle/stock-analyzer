use anyhow::{Context, bail};
use backoff::{Error as BackoffError, future::retry};
use futures::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use super::super::LlmClient;

#[derive(Debug, Serialize)]
pub(crate) struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ResponseFormat {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChatCompletionResponse {
    #[serde(default)]
    model: Option<String>,
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<ChatCompletionUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatCompletionUsage {
    #[serde(default)]
    prompt_tokens: i64,
    #[serde(default)]
    completion_tokens: i64,
    #[serde(default)]
    total_tokens: i64,
}

pub(crate) fn estimate_chat_completion_usage(
    request: &ChatCompletionRequest,
    content: &str,
) -> ChatCompletionUsage {
    let prompt_chars = request
        .messages
        .iter()
        .map(|message| message.role.len() + message.content.len())
        .sum::<usize>() as i64;
    let completion_chars = content.len() as i64;
    let prompt_tokens = approximate_tokens_from_chars(prompt_chars);
    let completion_tokens = approximate_tokens_from_chars(completion_chars);
    ChatCompletionUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    }
}

fn approximate_tokens_from_chars(chars: i64) -> i64 {
    ((chars.max(1) + 3) / 4).max(1)
}

#[derive(Debug, Deserialize)]
pub(crate) struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelItem>,
}

#[derive(Debug, Deserialize)]
struct ModelItem {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ChatMessageResponse {
    #[serde(default)]
    content: Option<Value>,
    #[serde(default)]
    refusal: Option<Value>,
    #[serde(default)]
    tool_calls: Option<Vec<Value>>,
}

impl ChatMessageResponse {
    fn content_text(&self) -> String {
        if let Some(content) = &self.content {
            match content {
                Value::String(text) => return text.clone(),
                Value::Array(parts) => {
                    let text = parts
                        .iter()
                        .filter_map(|part| match part {
                            Value::Object(map) => map
                                .get("text")
                                .and_then(Value::as_str)
                                .or_else(|| map.get("content").and_then(Value::as_str)),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.trim().is_empty() {
                        return text;
                    }
                }
                Value::Null => {}
                other => return other.to_string(),
            }
        }

        if let Some(refusal) = &self.refusal {
            match refusal {
                Value::String(text) if !text.trim().is_empty() => return text.clone(),
                Value::Array(parts) => {
                    let text = parts
                        .iter()
                        .filter_map(|part| match part {
                            Value::Object(map) => map.get("text").and_then(Value::as_str),
                            Value::String(text) => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.trim().is_empty() {
                        return text;
                    }
                }
                other if !other.is_null() => return other.to_string(),
                _ => {}
            }
        }

        if let Some(tool_calls) = &self.tool_calls
            && !tool_calls.is_empty()
        {
            return serde_json::to_string(tool_calls).unwrap_or_default();
        }

        String::new()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: Option<i64>,
    #[serde(default)]
    output_tokens: Option<i64>,
}

/// Shared exponential backoff configuration for LLM retries.
/// Initial: 750ms, multiplier: 2x, jitter: 15%, max interval: 8s, max elapsed: 120s.
pub(crate) fn llm_retry_backoff() -> backoff::ExponentialBackoff {
    backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(std::time::Duration::from_millis(750))
        .with_multiplier(2.0)
        .with_randomization_factor(0.15)
        .with_max_interval(std::time::Duration::from_secs(8))
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(120)))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- approximate_tokens_from_chars ---

    #[test]
    fn tokens_from_small_input() {
        assert_eq!(approximate_tokens_from_chars(1), 1);
    }

    #[test]
    fn tokens_from_medium_input() {
        // 100 chars → (100+3)/4 = 25
        assert_eq!(approximate_tokens_from_chars(100), 25);
    }

    #[test]
    fn tokens_from_zero() {
        // 0 chars → max(1, ...) → (1+3)/4 = 1
        assert_eq!(approximate_tokens_from_chars(0), 1);
    }

    // --- estimate_chat_completion_usage ---

    #[test]
    fn estimate_usage_basic() {
        let request = ChatCompletionRequest {
            model: "gpt-4".into(),
            messages: vec![
                ChatMessage { role: "user".into(), content: "hello world".into() },
            ],
            temperature: 0.7,
            response_format: None,
        };
        let usage = estimate_chat_completion_usage(&request, "response text");
        assert!(usage.prompt_tokens > 0);
        assert!(usage.completion_tokens > 0);
        assert_eq!(usage.total_tokens, usage.prompt_tokens + usage.completion_tokens);
    }

    #[test]
    fn estimate_usage_empty_response() {
        let request = ChatCompletionRequest {
            model: "gpt-4".into(),
            messages: vec![
                ChatMessage { role: "user".into(), content: "test".into() },
            ],
            temperature: 0.0,
            response_format: None,
        };
        let usage = estimate_chat_completion_usage(&request, "");
        assert!(usage.completion_tokens >= 1);
    }

    // --- ChatMessageResponse::content_text ---

    #[test]
    fn content_text_string() {
        let resp = ChatMessageResponse {
            content: Some(serde_json::json!("hello")),
            refusal: None,
            tool_calls: None,
        };
        assert_eq!(resp.content_text(), "hello");
    }

    #[test]
    fn content_text_array() {
        let resp = ChatMessageResponse {
            content: Some(serde_json::json!([{"text": "hello"}, {"text": "world"}])),
            refusal: None,
            tool_calls: None,
        };
        assert_eq!(resp.content_text(), "hello\nworld");
    }

    #[test]
    fn content_text_null() {
        let resp = ChatMessageResponse {
            content: Some(serde_json::json!(null)),
            refusal: None,
            tool_calls: None,
        };
        assert_eq!(resp.content_text(), "");
    }

    #[test]
    fn content_text_from_refusal() {
        let resp = ChatMessageResponse {
            content: None,
            refusal: Some(serde_json::json!("refused")),
            tool_calls: None,
        };
        assert_eq!(resp.content_text(), "refused");
    }

    #[test]
    fn content_text_from_tool_calls() {
        let resp = ChatMessageResponse {
            content: None,
            refusal: None,
            tool_calls: Some(vec![serde_json::json!({"function": "test"})]),
        };
        assert!(resp.content_text().contains("test"));
    }

    #[test]
    fn content_text_empty() {
        let resp = ChatMessageResponse {
            content: None,
            refusal: None,
            tool_calls: None,
        };
        assert_eq!(resp.content_text(), "");
    }

    // --- llm_retry_backoff ---

    #[test]
    fn retry_backoff_creates() {
        let backoff = llm_retry_backoff();
        // Just verify it doesn't panic
        drop(backoff);
    }

    // --- ModelsResponse deserialization ---

    #[test]
    fn models_response_deserialize() {
        let json = r#"{"data": [{"id": "gpt-4"}, {"id": "gpt-3.5-turbo"}]}"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].id, "gpt-4");
    }

    #[test]
    fn models_response_empty() {
        let json = r#"{"data": []}"#;
        let resp: ModelsResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
    }

    // --- ChatCompletionResponse deserialization ---

    #[test]
    fn chat_response_deserialize() {
        let json = r#"{"choices": [{"message": {"content": "hello"}}]}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.choices.len(), 1);
    }

    #[test]
    fn chat_response_with_usage() {
        let json = r#"{"choices": [{"message": {"content": "hi"}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        let usage = resp.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
    }
}

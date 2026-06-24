#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    model: Option<String>,
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<ChatCompletionUsage>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionUsage {
    #[serde(default)]
    prompt_tokens: i64,
    #[serde(default)]
    completion_tokens: i64,
    #[serde(default)]
    total_tokens: i64,
}

fn estimate_chat_completion_usage(
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
struct ModelsResponse {
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
struct ChatMessageResponse {
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
struct AnthropicResponse {
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
pub(super) fn llm_retry_backoff() -> backoff::ExponentialBackoff {
    backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(std::time::Duration::from_millis(750))
        .with_multiplier(2.0)
        .with_randomization_factor(0.15)
        .with_max_interval(std::time::Duration::from_secs(8))
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(120)))
        .build()
}

#[cfg(test)]
mod types_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn approximate_tokens_from_chars_short_text() {
        // 1 char -> (1+3)/4 = 1, max(1,1) = 1
        assert_eq!(approximate_tokens_from_chars(1), 1);
    }

    #[test]
    fn approximate_tokens_from_chars_100_chars() {
        // 100 chars -> (100+3)/4 = 25
        assert_eq!(approximate_tokens_from_chars(100), 25);
    }

    #[test]
    fn approximate_tokens_from_chars_zero() {
        // 0 chars -> max(1,0)=1 -> (1+3)/4=1
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
        };
        let usage = estimate_chat_completion_usage(&request, "Hi there");
        assert!(usage.prompt_tokens > 0);
        assert!(usage.completion_tokens > 0);
        assert_eq!(usage.total_tokens, usage.prompt_tokens + usage.completion_tokens);
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
        let json_str = r#"{"content":[{"text":"hello"}],"usage":{"input_tokens":10,"output_tokens":5}}"#;
        let resp: AnthropicResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(resp.content.len(), 1);
        assert_eq!(resp.content[0].text.as_deref(), Some("hello"));
        assert_eq!(resp.usage.input_tokens, Some(10));
        assert_eq!(resp.usage.output_tokens, Some(5));
    }
}

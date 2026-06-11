#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{Json, Router, routing::post};
    use serde_json::json;
    use tokio::net::TcpListener;

    use crate::llm::LlmClient;

    use super::{ChatMessageResponse, is_retryable_llm_error};

    #[test]
    fn retryable_llm_error_detection_only_matches_transient_failures() {
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 502 Bad Gateway: upstream ended"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "LLM request timed out"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "failed to call OpenAI-compatible LLM endpoint"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 524 <unknown status code>: error code: 524"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 520 <unknown status code>: error code: 520"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 522 <unknown status code>: error code: 522"
        )));
        assert!(is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 526 <unknown status code>: error code: 526"
        )));
        assert!(!is_retryable_llm_error(&anyhow::anyhow!(
            "llm request failed with 400 Bad Request: invalid schema"
        )));
        assert!(!is_retryable_llm_error(&anyhow::anyhow!(
            "LLM response contained no content"
        )));
    }

    #[test]
    fn content_text_tolerates_null_content_with_tool_calls() {
        let response = ChatMessageResponse {
            content: None,
            refusal: None,
            tool_calls: Some(vec![json!({
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "get_news",
                    "arguments": "{\"symbol\":\"NVDA\"}"
                }
            })]),
        };
        let text = response.content_text();
        assert!(text.contains("get_news"));
    }

    #[test]
    fn content_text_reads_text_parts_from_array() {
        let response = ChatMessageResponse {
            content: Some(json!([
                { "type": "output_text", "text": "{\"ok\":true}" }
            ])),
            refusal: None,
            tool_calls: None,
        };
        assert_eq!(response.content_text(), "{\"ok\":true}");
    }

    #[tokio::test]
    async fn openai_compatible_generation_recovers_from_transient_502s() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_state = attempts.clone();

        async fn flaky_chat_completion(
            axum::extract::State(attempts): axum::extract::State<Arc<AtomicUsize>>,
        ) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt < 3 {
                return Err((
                    axum::http::StatusCode::BAD_GATEWAY,
                    Json(json!({
                        "error": {
                            "message": "Upstream authentication failed, please contact administrator",
                            "type": "upstream_error"
                        }
                    })),
                ));
            }

            Ok(Json(json!({
                "choices": [{
                    "message": {
                        "content": "{\"ok\":true}"
                    }
                }]
            })))
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/chat/completions", post(flaky_chat_completion))
            .with_state(attempts_state);
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
        let client =
            LlmClient::openai_compatible(http, &format!("http://{addr}"), "test-key", "test-model", 30);

        let content = client
            .generate_with_openai_compatible("{\"probe\":true}")
            .await
            .unwrap();
        assert_eq!(content, "{\"ok\":true}");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }
}

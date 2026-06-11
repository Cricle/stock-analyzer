use serde::Deserialize;

use crate::engine::score::types::DimensionScore;

/// Sentiment scoring via LLM. Takes news headlines and returns a score.
pub async fn score_sentiment(
    llm: &crate::engine::llm::LlmClient,
    symbol: &str,
    headlines: &[String],
    news_limit: usize,
) -> DimensionScore {
    if headlines.is_empty() {
        return DimensionScore {
            score: 50,
            reason: "无新闻数据，情绪中性".into(),
        };
    }

    let limited: Vec<&str> = headlines.iter().take(news_limit).map(String::as_str).collect();
    let news_text = limited.join("\n- ");
    let prompt = format!(
        "给股票 {symbol} 的近期新闻情绪评分。评分范围 0-100，50=中性，>70 积极，<30 消极。\n\
         只输出 JSON，不要其他内容：\n\
         {{\"score\": <0-100>, \"reason\": \"一句话理由\"}}\n\n\
         新闻标题：\n- {news_text}"
    );

    let content = match llm.generate(&prompt).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(symbol = %symbol, error = %e, "sentiment LLM call failed");
            return DimensionScore {
                score: 50,
                reason: format!("情绪分析LLM调用失败: {e}"),
            };
        }
    };

    parse_sentiment_response(&content)
}

#[derive(Deserialize)]
struct SentimentResponse {
    score: u8,
    reason: String,
}

fn parse_sentiment_response(content: &str) -> DimensionScore {
    let json_str = content
        .trim()
        .strip_prefix("```json")
        .and_then(|s| s.strip_suffix("```"))
        .or_else(|| content.strip_prefix("```").and_then(|s| s.strip_suffix("```")))
        .unwrap_or(content.trim());

    match serde_json::from_str::<SentimentResponse>(json_str) {
        Ok(resp) => DimensionScore {
            score: resp.score.clamp(0, 100),
            reason: resp.reason,
        },
        Err(e) => {
            tracing::warn!(error = %e, raw = %content, "failed to parse sentiment JSON");
            DimensionScore {
                score: 50,
                reason: "情绪分析解析失败，使用中性评分".into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_json() {
        let raw = r#"{"score": 75, "reason": "近期利好消息较多"}"#;
        let result = parse_sentiment_response(raw);
        assert_eq!(result.score, 75);
        assert!(result.reason.contains("利好"));
    }

    #[test]
    fn test_parse_json_in_codeblock() {
        let raw = "```json\n{\"score\": 30, \"reason\": \"利空\"}\n```";
        let result = parse_sentiment_response(raw);
        assert_eq!(result.score, 30);
    }

    #[test]
    fn test_parse_invalid_returns_neutral() {
        let raw = "I cannot provide a score";
        let result = parse_sentiment_response(raw);
        assert_eq!(result.score, 50);
    }

    #[test]
    fn test_parse_score_over_100_clamped() {
        let raw = r#"{"score": 150, "reason": "超出范围"}"#;
        let result = parse_sentiment_response(raw);
        assert_eq!(result.score, 100, "score should be clamped to 100");
    }

    #[test]
    fn test_parse_score_under_0_clamped() {
        let raw = r#"{"score": 0, "reason": "最低分"}"#;
        let result = parse_sentiment_response(raw);
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_parse_json_with_extra_whitespace() {
        let raw = "  \n  {\"score\": 60, \"reason\": \"偏积极\"}  \n  ";
        let result = parse_sentiment_response(raw);
        assert_eq!(result.score, 60);
    }

    #[test]
    fn test_parse_json_with_trailing_text() {
        let raw = r#"{"score": 40, "reason": "偏消极"} some extra text"#;
        let result = parse_sentiment_response(raw);
        // Should fail to parse and return neutral
        assert_eq!(result.score, 50);
    }
}

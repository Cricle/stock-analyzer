fn parse_object_candidates_value<R>(content: &str, mapper: fn(Value) -> R) -> anyhow::Result<R> {
    let candidates = candidate_variants(content);

    let mut last_error = None;
    for candidate in candidates {
        match serde_json::from_str::<Value>(&candidate) {
            Ok(raw) => return Ok(mapper(raw)),
            Err(error) => last_error = Some(error),
        }
    }

    bail!(
        "{}",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "unknown JSON parsing error".to_string())
    )
}

fn parse_generated_debate_turn_lenient(content: &str) -> Option<GeneratedDebateTurn> {
    for candidate in candidate_variants(content) {
        if let Some(parsed) = parse_generated_debate_turn_lenient_candidate(&candidate) {
            return Some(parsed);
        }
    }
    None
}

fn parse_generated_debate_turn_lenient_candidate(content: &str) -> Option<GeneratedDebateTurn> {
    let speaker = extract_simple_json_string_field(content, "speaker")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Unknown".to_string());
    let stance = extract_simple_json_string_field(content, "stance")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "neutral".to_string());
    let response_raw = extract_relaxed_json_string_field(
        content,
        "response",
        &["confidence", "evidence_points", "risks"],
    )
    .filter(|value| !value.is_empty());
    let (response, response_key) = match response_raw {
        Some(r) => (r, None),
        None => (
            String::new(),
            Some("llm.fallback.no_debate".to_string()),
        ),
    };
    let confidence =
        extract_json_value_before_known_field(content, "confidence", &["evidence_points", "risks"])
            .unwrap_or(Value::String("unknown".to_string()));
    let evidence_points_raw =
        extract_json_value_before_known_field(content, "evidence_points", &["risks"]);
    let (evidence_points, evidence_points_key) = match evidence_points_raw {
        Some(value) => {
            let list = string_list_or_default(Some(value), &[]);
            (
                list.clone(),
                if list.is_empty() {
                    Some("llm.fallback.no_evidence".to_string())
                } else {
                    None
                },
            )
        }
        None => (
            Vec::new(),
            Some("llm.fallback.no_evidence".to_string()),
        ),
    };
    let risks_raw = extract_json_value_before_known_field(content, "risks", &[]);
    let (risks, risks_key) = match risks_raw {
        Some(value) => {
            let list = string_list_or_default(Some(value), &[]);
            (
                list.clone(),
                if list.is_empty() {
                    Some("llm.fallback.no_risk".to_string())
                } else {
                    None
                },
            )
        }
        None => (
            Vec::new(),
            Some("llm.fallback.no_risk".to_string()),
        ),
    };

    Some(GeneratedDebateTurn {
        speaker,
        stance,
        response,
        response_key,
        confidence,
        evidence_points,
        evidence_points_key,
        risks,
        risks_key,
    })
}

/// Replace bare control characters (U+0000..U+001F) inside JSON string
/// values with their properly escaped equivalents.  This handles LLM output
/// that contains literal newlines, tabs, or null bytes inside strings, which
/// `serde_json` rejects by default.
fn sanitize_json_control_chars(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() + 32);
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if escaped {
            out.push(b);
            escaped = false;
            i += 1;
            continue;
        }
        if in_string {
            match b {
                b'\\' => {
                    out.push(b);
                    escaped = true;
                    i += 1;
                }
                b'"' => {
                    out.push(b);
                    in_string = false;
                    i += 1;
                }
                0x00..=0x1f => {
                    match b {
                        b'\n' => out.extend_from_slice(b"\\n"),
                        b'\r' => out.extend_from_slice(b"\\r"),
                        b'\t' => out.extend_from_slice(b"\\t"),
                        0x08 => out.extend_from_slice(b"\\b"),
                        0x0c => out.extend_from_slice(b"\\f"),
                        other => {
                            let s = format!("\\u{:04x}", other); out.extend_from_slice(s.as_bytes());
                        }
                    }
                    i += 1;
                }
                _ => {
                    out.push(b);
                    i += 1;
                }
            }
        } else {
            if b == b'"' {
                in_string = true;
            }
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| content.to_string())
}

fn candidate_variants(content: &str) -> Vec<String> {
    let trimmed = content.trim();
    let sanitized = sanitize_json_control_chars(trimmed);
    let mut seeds = vec![sanitized, trimmed.to_string()];

    if let Some(unfenced) = strip_code_fence(trimmed) {
        seeds.push(unfenced.to_string());
    }

    if let Some(braced) = slice_outer_json_object(trimmed) {
        seeds.push(braced.to_string());
    }

    if let Some(first_json) = slice_first_complete_json_value(trimmed) {
        seeds.push(first_json.to_string());
    }

    let mut candidates = Vec::new();
    for seed in seeds {
        candidates.push(seed.clone());
        if let Some(repaired) = repair_common_malformed_json_variants(&seed)
            && repaired != seed {
                candidates.push(repaired);
            }
    }

    candidates
}


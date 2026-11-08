use super::providers::request;
use super::*;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn review(task: AiTask) -> AiReview {
    context::prepare(
        task,
        "A test",
        None,
        Some(
            json!({"candidate":{"metrics":{"request_rate":12.0},"measurement":{"complete":false}}}),
        ),
    )
    .unwrap()
}

fn answer(definition: Value) -> Value {
    json!({"definition":definition,"observations":[{"text":"Observed throughput was 12 requests/s.","evidence":["/candidate/metrics/request_rate"]}],"hypotheses":[],"next_steps":[],"assumptions":[],"missing_inputs":[],"unsupported":[]})
}

#[test]
fn evidence_pointers_must_resolve_to_shared_measurements() {
    let review = review(AiTask::Explain);
    let source = answer(Value::Null);
    assert!(parse_answer(&source.to_string(), &review, "model", "private-key").is_ok());
    for pointer in [
        "/candidate/missing",
        "/candidate/metrics",
        "",
        "/description",
    ] {
        let mut source = source.clone();
        source["observations"][0]["evidence"] = json!([pointer]);
        assert!(parse_answer(&source.to_string(), &review, "model", "private-key").is_err());
    }
    let mut source = source;
    source["observations"][0]["evidence"] = json!([]);
    assert!(parse_answer(&source.to_string(), &review, "model", "private-key").is_err());
}

#[test]
fn draft_schema_credentials_and_budget_are_enforced() {
    let review = review(AiTask::Draft);
    let mut definition = serde_json::to_value(TestDefinition::default()).unwrap();
    definition["config"]["budget"]["max_vus"] = json!(99999);
    let result = parse_answer(
        &answer(definition.clone()).to_string(),
        &review,
        "model",
        "private-key",
    )
    .unwrap();
    assert_eq!(
        result.definition.unwrap().config.budget.max_vus,
        TestDefinition::default().config.budget.max_vus
    );
    definition["unsupported_field"] = json!(true);
    assert!(
        parse_answer(
            &answer(definition).to_string(),
            &review,
            "model",
            "private-key"
        )
        .is_err()
    );
    let mut definition = serde_json::to_value(TestDefinition::default()).unwrap();
    definition["config"]["auth"] = json!({"kind":"bearer","token":"invented"});
    assert!(
        parse_answer(
            &answer(definition).to_string(),
            &review,
            "model",
            "private-key"
        )
        .is_err()
    );
    assert!(
        parse_answer(
            &answer(Value::Null).to_string(),
            &review,
            "model",
            "private-key"
        )
        .is_err()
    );
}

#[test]
fn provider_key_cannot_be_returned_in_the_answer() {
    let review = review(AiTask::Explain);
    let mut source = answer(Value::Null);
    source["hypotheses"] = json!(["private-key"]);
    assert!(parse_answer(&source.to_string(), &review, "model", "private-key").is_err());
}

#[test]
fn reviews_are_frozen_consumed_once_and_generation_owns_cancellation() {
    let state = AiState::default();
    let mut review = review(AiTask::Explain);
    let id = review.id.clone();
    let expected = review.content.clone();
    let expected_config = review.config.clone();
    state
        .inner
        .lock()
        .unwrap()
        .reviews
        .insert(id.clone(), review.clone());
    review.config.model = "changed-after-review".into();
    review.config.endpoint = "https://unreviewed.example/chat".into();
    let (generation, selected) = Generation::begin(&state, &id).unwrap();
    assert_eq!(selected.content, expected);
    assert_eq!(selected.config, expected_config);
    assert!(Generation::begin(&state, &id).is_err());
    let token = generation.cancelled.clone();
    token.cancel();
    assert!(generation.cancelled.is_cancelled());
    drop(generation);
    assert!(state.inner.lock().unwrap().active.is_none());
    assert!(Generation::begin(&state, &id).is_err());
}

async fn server(response: String) -> (String, tokio::task::JoinHandle<String>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = vec![0; 16384];
        let count = stream.read(&mut bytes).await.unwrap();
        stream.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8_lossy(&bytes[..count]).into_owned()
    });
    (format!("http://{address}"), task)
}

#[tokio::test]
async fn transport_rejects_redirects_and_omits_provider_error_bodies() {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    for status in [
        "302 Found\r\nLocation: http://127.0.0.1:1",
        "401 Unauthorized",
    ] {
        let (endpoint, captured) = server(format!(
            "HTTP/1.1 {status}\r\nContent-Length: 16\r\nConnection: close\r\n\r\nPRIVATE_RESPONSE"
        ))
        .await;
        let error = request(
            &client,
            &local_config(&endpoint),
            Some("private-key"),
            "system",
            "{}",
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "AI_PROVIDER_UNAVAILABLE");
        assert!(!error.message.contains("PRIVATE_RESPONSE"));
        assert!(captured.await.unwrap().contains("Bearer private-key"));
    }
}

#[tokio::test]
async fn transport_rejects_oversized_and_incomplete_completions() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let (endpoint, task) = server(format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        MAX_RESPONSE_BYTES + 1
    ))
    .await;
    assert_eq!(
        request(
            &client,
            &local_config(&endpoint),
            Some("private-key"),
            "system",
            "{}"
        )
        .await
        .unwrap_err()
        .code,
        "AI_RESPONSE_TOO_LARGE"
    );
    task.await.unwrap();
    let body =
        json!({"choices":[{"message":{"content":"{}"},"finish_reason":"length"}]}).to_string();
    let (endpoint, task) = server(format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ))
    .await;
    assert_eq!(
        request(
            &client,
            &local_config(&endpoint),
            Some("private-key"),
            "system",
            "{}"
        )
        .await
        .unwrap_err()
        .code,
        "AI_INVALID_RESPONSE"
    );
    task.await.unwrap();
}

#[test]
fn cancellation_cannot_affect_another_panels_generation() {
    let state = AiState::default();
    let first = review(AiTask::Explain);
    let second = review(AiTask::Draft);
    let first_id = first.id.clone();
    let second_id = second.id.clone();
    {
        let mut session = state.inner.lock().unwrap();
        session.reviews.insert(first_id.clone(), first);
        session.reviews.insert(second_id.clone(), second);
    }
    let (generation, _) = Generation::begin(&state, &first_id).unwrap();
    cancel_review(&state, &second_id).unwrap();
    assert!(!generation.cancelled.is_cancelled());
    assert!(!state.inner.lock().unwrap().reviews.contains_key(&second_id));
    cancel_review(&state, &first_id).unwrap();
    assert!(generation.cancelled.is_cancelled());
}

#[test]
fn json_escaping_cannot_hide_keys_or_scenario_credentials() {
    let review = review(AiTask::Explain);
    let source = answer(Value::Null).to_string().replace(
        "Observed throughput",
        "privat\\u0065-key Observed throughput",
    );
    assert!(parse_answer(&source, &review, "model", "private-key").is_err());
    assert!(has_unsafe_fields(
        &json!({"scenarios":{"steps":[{"request":{"auth":{"kind":"bearer","token":"invented"}}}]}})
    ));
    assert!(has_unsafe_fields(
        &json!({"headers":[{"name":"X-API-Key","value":"invented"}]})
    ));
    assert!(has_unsafe_fields(
        &json!({"url":"https://example.com?token=invented"})
    ));
    assert!(!has_unsafe_fields(
        &json!({"headers":[{"name":"Content-Type","value":"application/json"}],"auth":{"kind":"none"}})
    ));
}

#[tokio::test]
async fn refusal_and_missing_messages_fail_without_provider_content() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    for body in [
        json!({"choices":[{"message":{"content":null,"refusal":"PRIVATE_REFUSAL"},"finish_reason":"stop"}]}),
        json!({"choices":[]}),
    ] {
        let body = body.to_string();
        let (endpoint, task) = server(format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ))
        .await;
        let error = request(
            &client,
            &local_config(&endpoint),
            Some("private-key"),
            "system",
            "{}",
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "AI_INVALID_RESPONSE");
        assert!(!error.message.contains("PRIVATE_REFUSAL"));
        task.await.unwrap();
    }
}

#[test]
fn native_discloses_missing_and_incomplete_measurements() {
    let review = review(AiTask::Explain);
    let result = parse_answer(
        &answer(Value::Null).to_string(),
        &review,
        "model",
        "private-key",
    )
    .unwrap();
    assert!(
        result
            .missing_inputs
            .iter()
            .any(|message| message.contains("candidate measurement window is incomplete"))
    );
    assert!(
        result
            .missing_inputs
            .iter()
            .any(|message| message.contains("No baseline measurements"))
    );
}

#[test]
fn comparison_limitations_survive_redaction_and_are_disclosed() {
    let review = context::prepare(
        AiTask::Explain,
        "Explain selected differences",
        None,
        Some(json!({
            "comparison_limited": true,
            "configuration_reviewed": false,
            "configuration_differences": 2,
            "limitations": ["PRIVATE_CONFIGURATION"],
        })),
    )
    .unwrap();
    assert!(!review.content.contains("PRIVATE_CONFIGURATION"));
    let selected: Value = serde_json::from_str(&review.content).unwrap();
    assert_eq!(selected["evidence"]["comparison_limited"], true);
    let parsed = parse_answer(
        &json!({
            "definition": null, "observations": [], "hypotheses": [], "next_steps": [],
            "assumptions": [], "missing_inputs": [], "unsupported": [],
        })
        .to_string(),
        &review,
        "model",
        "private-key",
    )
    .unwrap();
    assert!(
        parsed
            .missing_inputs
            .iter()
            .any(|entry| entry.contains("comparison has compatibility"))
    );
}

fn local_config(endpoint: &str) -> AiProviderConfig {
    AiProviderConfig {
        provider: AiProvider::OpenaiCompatible,
        model: "model".into(),
        endpoint: endpoint.into(),
        use_key: true,
    }
}

#[test]
fn keyless_responses_are_validated_and_provider_provenance_is_native() {
    let mut review = review(AiTask::Explain);
    review.config = AiProviderConfig {
        provider: AiProvider::Ollama,
        model: "llama3:latest".into(),
        endpoint: "http://localhost:11434/api/chat".into(),
        use_key: false,
    };
    let answer = parse_answer(
        &answer(Value::Null).to_string(),
        &review,
        "llama3:latest",
        "",
    )
    .unwrap();
    assert_eq!(answer.provider, "Ollama");
    assert_eq!(answer.endpoint, "http://localhost:11434/api/chat");
}

#[test]
fn returned_model_identity_cannot_expose_a_provider_key() {
    let review = review(AiTask::Explain);
    assert!(
        parse_answer(
            &answer(Value::Null).to_string(),
            &review,
            "model-private-key",
            "private-key"
        )
        .is_err()
    );
}

#[test]
fn credential_values_have_one_bounded_validation_rule() {
    assert!(valid_key("provider-key-123"));
    for value in ["", "with space", "key\nnext", "é", &"k".repeat(4097)] {
        assert!(!valid_key(value));
    }
}

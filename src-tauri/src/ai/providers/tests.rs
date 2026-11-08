use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn config(provider: AiProvider, endpoint: &str) -> AiProviderConfig {
    AiProviderConfig {
        provider,
        endpoint: endpoint.into(),
        model: "test-model".into(),
        use_key: provider.requires_key(),
    }
}

#[test]
fn destinations_are_fixed_or_explicitly_secure_custom_endpoints() {
    for provider in [
        AiProvider::Openai,
        AiProvider::Anthropic,
        AiProvider::Gemini,
    ] {
        let selected = config(provider, provider.fixed_endpoint().unwrap());
        assert!(validate(selected.clone()).is_ok());
        assert!(endpoint(provider, "https://attacker.example/api").is_err());
        let mut no_key = selected;
        no_key.use_key = false;
        assert_eq!(validate(no_key).unwrap_err().code, "AI_KEY_REQUIRED");
    }
    for valid in [
        "http://localhost:11434/api/chat",
        "http://127.0.0.1:8080/v1/chat/completions",
        "http://[::1]:11434/api/chat",
        "https://gateway.example/custom/v1/chat/completions",
    ] {
        assert!(endpoint(AiProvider::OpenaiCompatible, valid).is_ok());
    }
    for invalid in [
        "http://remote.example/chat",
        "http://192.168.0.1/chat",
        "file:///private/file",
        "https://user:password@example.com/chat",
        "https://example.com/chat?key=PRIVATE",
        "https://example.com/chat#fragment",
        "http://localhost:0/chat",
        "https://localhost.evil/chat?",
        " https://example.com/chat",
    ] {
        assert!(endpoint(AiProvider::Ollama, invalid).is_err(), "{invalid}");
    }
    let ipv4_numeric = endpoint(AiProvider::Ollama, "http://2130706433:11434/api/chat").unwrap();
    assert_eq!(ipv4_numeric, "http://127.0.0.1:11434/api/chat");
}

#[test]
fn credentials_are_scoped_to_normalized_provider_and_exact_endpoint() {
    let first =
        credential_id(AiProvider::OpenaiCompatible, "https://EXAMPLE.com:443/chat").unwrap();
    assert_eq!(
        first,
        credential_id(AiProvider::OpenaiCompatible, "https://example.com/chat").unwrap()
    );
    assert_ne!(
        first,
        credential_id(AiProvider::Ollama, "https://example.com/chat").unwrap()
    );
    assert_ne!(
        first,
        credential_id(AiProvider::OpenaiCompatible, "https://example.com/other").unwrap()
    );
    assert!(!first.contains("example.com"));
    let mut selected = config(AiProvider::Ollama, "http://localhost:11434/api/chat");
    for model in ["qwen3:8b", "hf.co/company/model:latest"] {
        selected.model = model.into();
        assert!(validate(selected.clone()).is_ok());
    }
    for model in [
        "",
        "../model",
        "models/../private",
        "model?api_key=PRIVATE",
        "model\nPRIVATE",
    ] {
        selected.model = model.into();
        assert!(validate(selected.clone()).is_err());
    }
    let mut gemini = config(AiProvider::Gemini, GEMINI_ENDPOINT);
    gemini.model = "models/untrusted:thing".into();
    assert_eq!(
        destination(&gemini).unwrap().path(),
        "/v1beta/models/models%2Funtrusted:thing:generateContent"
    );
}

fn success(provider: AiProvider) -> Value {
    match provider {
        AiProvider::Openai | AiProvider::OpenaiCompatible => {
            json!({"model":"actual-model","choices":[{"message":{"role":"assistant","content":"{}"},"finish_reason":"stop"}]})
        }
        AiProvider::Anthropic => {
            json!({"model":"actual-model","role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":"{}"}]})
        }
        AiProvider::Gemini => {
            json!({"modelVersion":"actual-model","candidates":[{"finishReason":"STOP","content":{"role":"model","parts":[{"text":"{}"}]}}]})
        }
        AiProvider::Ollama => {
            json!({"model":"actual-model","done":true,"done_reason":"stop","message":{"role":"assistant","content":"{}"}})
        }
    }
}

async fn server(
    response: Value,
) -> (
    String,
    tokio::task::JoinHandle<Value>,
    tokio::sync::oneshot::Receiver<String>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (send_headers, headers) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0; 4096];
        let (header_end, length) = loop {
            let count = stream.read(&mut buffer).await.unwrap();
            assert_ne!(count, 0);
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let head = String::from_utf8(bytes[..index].to_vec()).unwrap();
                let length = head
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .unwrap();
                send_headers.send(head).unwrap();
                break (index + 4, length);
            }
        };
        while bytes.len() < header_end + length {
            let count = stream.read(&mut buffer).await.unwrap();
            assert_ne!(count, 0);
            bytes.extend_from_slice(&buffer[..count]);
        }
        let content = response.to_string();
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{content}",
                    content.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap()
    });
    (endpoint, task, headers)
}

#[tokio::test]
async fn every_adapter_sends_its_native_protocol_and_extracts_text_provenance() {
    for provider in [
        AiProvider::Openai,
        AiProvider::Anthropic,
        AiProvider::Gemini,
        AiProvider::Ollama,
        AiProvider::OpenaiCompatible,
    ] {
        let (endpoint, task, headers) = server(success(provider)).await;
        let selected = config(provider, &endpoint);
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let answer = request(
            &client,
            &selected,
            Some("PRIVATE-KEY"),
            "fixed system",
            "reviewed context",
        )
        .await
        .unwrap();
        assert_eq!(answer.content, "{}");
        assert_eq!(answer.model, "actual-model");
        let headers = headers.await.unwrap().to_ascii_lowercase();
        assert!(!headers.lines().next().unwrap().contains("private-key"));
        let body = task.await.unwrap();
        assert!(!body.to_string().contains("PRIVATE-KEY"));
        match provider {
            AiProvider::Openai | AiProvider::OpenaiCompatible => {
                assert!(headers.contains("authorization: bearer private-key"));
                assert_eq!(body["response_format"]["type"], "json_object");
                assert_eq!(body["messages"][1]["content"], "reviewed context");
            }
            AiProvider::Anthropic => {
                assert!(headers.contains("authorization: bearer private-key"));
                assert!(headers.contains("anthropic-version: 2026-10-21"));
                assert_eq!(body["system"], "fixed system");
                assert_eq!(body["max_tokens"], 8192);
            }
            AiProvider::Gemini => {
                assert!(headers.contains("x-goog-api-key: private-key"));
                assert!(
                    headers
                        .lines()
                        .next()
                        .unwrap()
                        .contains("/models/test-model:generatecontent")
                );
                assert_eq!(
                    body["systemInstruction"]["parts"][0]["text"],
                    "fixed system"
                );
                assert_eq!(
                    body["generationConfig"]["responseMimeType"],
                    "application/json"
                );
                assert!(body.get("store").is_none());
            }
            AiProvider::Ollama => {
                assert_eq!(body["stream"], false);
                assert_eq!(body["format"], "json");
            }
        }
    }
}

#[test]
fn adapters_reject_refusal_tools_truncation_and_ambiguous_responses() {
    let mut cases = Vec::new();
    for provider in [AiProvider::Openai, AiProvider::OpenaiCompatible] {
        let mut source = success(provider);
        source["choices"][0]["message"]["tool_calls"] = json!([{"name":"start_run"}]);
        cases.push((provider, source));
        let mut source = success(provider);
        source["choices"][0]["message"]["refusal"] = json!("PRIVATE_REFUSAL");
        cases.push((provider, source));
        let mut source = success(provider);
        source["choices"][0]["finish_reason"] = json!("length");
        cases.push((provider, source));
        let mut source = success(provider);
        let second = source["choices"][0].clone();
        source["choices"].as_array_mut().unwrap().push(second);
        cases.push((provider, source));
    }
    let mut source = success(AiProvider::Anthropic);
    source["stop_details"] = json!({"type":"refusal","explanation":"PRIVATE_REFUSAL"});
    cases.push((AiProvider::Anthropic, source));
    let mut source = success(AiProvider::Anthropic);
    source["content"] = json!([{"type":"tool_use","name":"start_run"}]);
    cases.push((AiProvider::Anthropic, source));
    let mut source = success(AiProvider::Gemini);
    source["candidates"][0]["finishReason"] = json!("MAX_TOKENS");
    cases.push((AiProvider::Gemini, source));
    let mut source = success(AiProvider::Gemini);
    source["promptFeedback"] = json!({"blockReason":"SAFETY"});
    cases.push((AiProvider::Gemini, source));
    let mut source = success(AiProvider::Gemini);
    source["candidates"][0]["content"]["parts"] =
        json!([{"functionCall":{"name":"start_run"},"text":"{}"}]);
    cases.push((AiProvider::Gemini, source));
    let mut source = success(AiProvider::Ollama);
    source["done_reason"] = json!("length");
    cases.push((AiProvider::Ollama, source));
    for (provider, value) in cases {
        let error = decode(&config(provider, "http://localhost"), &value).unwrap_err();
        assert_eq!(error.code, "AI_INVALID_RESPONSE");
        assert!(!error.message.contains("PRIVATE_REFUSAL"));
    }
}

#[tokio::test]
async fn keyless_local_calls_send_no_authentication_header() {
    for provider in [AiProvider::Ollama, AiProvider::OpenaiCompatible] {
        let (endpoint, task, headers) = server(success(provider)).await;
        let selected = config(provider, &endpoint);
        let response = request(
            &client(&selected).unwrap(),
            &selected,
            None,
            "system",
            "context",
        )
        .await
        .unwrap();
        assert_eq!(response.content, "{}");
        let headers = headers.await.unwrap().to_ascii_lowercase();
        assert!(!headers.contains("authorization:"));
        assert!(!headers.contains("x-goog-api-key:"));
        task.await.unwrap();
    }
}

#[test]
fn reasoning_is_not_exposed_as_answer_text_and_model_identity_is_bounded() {
    let mut gemini = success(AiProvider::Gemini);
    gemini["promptFeedback"] = json!({"blockReason":"BLOCK_REASON_UNSPECIFIED"});
    assert!(decode(&config(AiProvider::Gemini, GEMINI_ENDPOINT), &gemini).is_ok());
    gemini["candidates"][0]["content"]["parts"] =
        json!([{"thought":true,"text":"PRIVATE_REASONING"},{"text":"{}"}]);
    assert_eq!(
        decode(&config(AiProvider::Gemini, GEMINI_ENDPOINT), &gemini)
            .unwrap()
            .content,
        "{}"
    );
    gemini["candidates"][0]["content"]["parts"] =
        json!([{"thought":true,"functionResponse":{"name":"start_run"}},{"text":"{}"}]);
    assert!(decode(&config(AiProvider::Gemini, GEMINI_ENDPOINT), &gemini).is_err());
    let mut anthropic = success(AiProvider::Anthropic);
    anthropic["content"] =
        json!([{"type":"thinking","thinking":"PRIVATE_REASONING"},{"type":"text","text":"{}"}]);
    assert_eq!(
        decode(
            &config(AiProvider::Anthropic, ANTHROPIC_ENDPOINT),
            &anthropic
        )
        .unwrap()
        .content,
        "{}"
    );
    for model in ["", "PRIVATE\nMODEL", &"x".repeat(201)] {
        let mut openai = success(AiProvider::Openai);
        openai["model"] = json!(model);
        assert!(decode(&config(AiProvider::Openai, OPENAI_ENDPOINT), &openai).is_err());
    }
}

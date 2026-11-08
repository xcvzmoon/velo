//! Provider protocols, destination validation, and bounded response decoding.

use std::{net::IpAddr, time::Duration};

use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use velo_engine::RunError;

use super::{MAX_RESPONSE_BYTES, failure};

const OPENAI_ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";
const ANTHROPIC_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const GEMINI_ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AiProvider {
    Openai,
    Anthropic,
    Gemini,
    Ollama,
    OpenaiCompatible,
}

impl AiProvider {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::Ollama => "ollama",
            Self::OpenaiCompatible => "openai_compatible",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Openai => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
            Self::Ollama => "Ollama",
            Self::OpenaiCompatible => "OpenAI-compatible",
        }
    }

    fn fixed_endpoint(self) -> Option<&'static str> {
        match self {
            Self::Openai => Some(OPENAI_ENDPOINT),
            Self::Anthropic => Some(ANTHROPIC_ENDPOINT),
            Self::Gemini => Some(GEMINI_ENDPOINT),
            Self::Ollama | Self::OpenaiCompatible => None,
        }
    }

    fn requires_key(self) -> bool {
        self.fixed_endpoint().is_some()
    }
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct AiProviderConfig {
    pub provider: AiProvider,
    pub model: String,
    pub endpoint: String,
    pub use_key: bool,
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            provider: AiProvider::Openai,
            model: "gpt-4.1-mini".into(),
            endpoint: OPENAI_ENDPOINT.into(),
            use_key: true,
        }
    }
}

fn invalid_endpoint() -> RunError {
    failure(
        "AI_INVALID_ENDPOINT",
        "Use the provider's fixed endpoint, or a custom HTTPS endpoint. HTTP is allowed only on localhost or a loopback IP. URLs cannot contain credentials, queries, or fragments.",
    )
}

fn loopback(url: &Url) -> bool {
    match url.host_str() {
        Some("localhost") => true,
        Some(host) => host
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback()),
        None => false,
    }
}

pub(super) fn endpoint(provider: AiProvider, source: &str) -> Result<String, RunError> {
    if source.len() > 2048
        || source.is_empty()
        || source.trim() != source
        || source.chars().any(char::is_control)
    {
        return Err(invalid_endpoint());
    }
    let parsed = Url::parse(source).map_err(|_| invalid_endpoint())?;
    if parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.port() == Some(0)
        || !(parsed.scheme() == "https" || parsed.scheme() == "http" && loopback(&parsed))
    {
        return Err(invalid_endpoint());
    }
    let normalized = parsed.to_string();
    if provider
        .fixed_endpoint()
        .is_some_and(|expected| normalized != expected)
    {
        return Err(invalid_endpoint());
    }
    Ok(normalized)
}

pub(super) fn validate(mut config: AiProviderConfig) -> Result<AiProviderConfig, RunError> {
    if config.model.is_empty()
        || config.model.len() > 200
        || !config.model.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
        || config.model.starts_with('/')
        || config.model.ends_with('/')
        || config
            .model
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(failure(
            "AI_INVALID_MODEL",
            "Enter a model identifier using letters, digits, dots, hyphens, underscores, colons, or slash-separated names.",
        ));
    }
    if config.provider.requires_key() && !config.use_key {
        return Err(failure(
            "AI_KEY_REQUIRED",
            "This provider requires a stored API key.",
        ));
    }
    config.endpoint = endpoint(config.provider, &config.endpoint)?;
    Ok(config)
}

pub(super) fn credential_id(provider: AiProvider, endpoint: &str) -> Result<String, RunError> {
    let normalized = self::endpoint(provider, endpoint)?;
    Ok(format!(
        "{}:{:x}",
        provider.id(),
        Sha256::digest(normalized.as_bytes())
    ))
}

pub(super) fn destination(config: &AiProviderConfig) -> Result<Url, RunError> {
    let mut url = Url::parse(&config.endpoint).map_err(|_| invalid_endpoint())?;
    if config.provider == AiProvider::Gemini {
        url.path_segments_mut()
            .map_err(|_| invalid_endpoint())?
            .push("models")
            .push(&format!("{}:generateContent", config.model));
    }
    Ok(url)
}

pub(super) fn client(config: &AiProviderConfig) -> Result<Client, RunError> {
    let target = destination(config)?;
    let mut builder = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10));
    if loopback(&target) {
        builder = builder.no_proxy();
    }
    builder.build().map_err(|_| {
        failure(
            "AI_UNAVAILABLE",
            "The provider connection could not be initialized.",
        )
    })
}

fn body(config: &AiProviderConfig, system: &str, content: &str) -> Value {
    match config.provider {
        AiProvider::Openai => {
            json!({"model":config.model,"store":false,"max_completion_tokens":8192,"response_format":{"type":"json_object"},"messages":[{"role":"system","content":system},{"role":"user","content":content}]})
        }
        AiProvider::OpenaiCompatible => {
            json!({"model":config.model,"stream":false,"max_tokens":8192,"response_format":{"type":"json_object"},"messages":[{"role":"system","content":system},{"role":"user","content":content}]})
        }
        AiProvider::Anthropic => {
            json!({"model":config.model,"max_tokens":8192,"stream":false,"system":system,"messages":[{"role":"user","content":content}]})
        }
        AiProvider::Gemini => {
            json!({"systemInstruction":{"parts":[{"text":system}]},"contents":[{"role":"user","parts":[{"text":content}]}],"generationConfig":{"responseMimeType":"application/json","maxOutputTokens":8192}})
        }
        AiProvider::Ollama => {
            json!({"model":config.model,"stream":false,"format":"json","options":{"num_predict":8192},"messages":[{"role":"system","content":system},{"role":"user","content":content}]})
        }
    }
}

fn build_request(
    client: &Client,
    config: &AiProviderConfig,
    key: Option<&str>,
    system: &str,
    content: &str,
) -> Result<RequestBuilder, RunError> {
    let mut request = client
        .post(destination(config)?)
        .header("Content-Type", "application/json");
    if config.provider == AiProvider::Anthropic {
        request = request.header("anthropic-version", "2026-10-21");
    }
    if let Some(key) = key {
        request = match config.provider {
            AiProvider::Gemini => request.header("x-goog-api-key", key),
            _ => request.bearer_auth(key),
        };
    }
    Ok(request.body(body(config, system, content).to_string()))
}

fn invalid_response() -> RunError {
    failure(
        "AI_INVALID_RESPONSE",
        "The provider returned an incomplete, refused, or unsupported response. Existing tests and results are unchanged.",
    )
}

fn optional_model(value: &Value, requested: &str) -> Result<String, RunError> {
    let model = value.get("model").or_else(|| value.get("modelVersion"));
    match model {
        None => Ok(requested.into()),
        Some(Value::String(model))
            if !model.is_empty()
                && model.len() <= 200
                && model.bytes().all(|byte| byte.is_ascii_graphic()) =>
        {
            Ok(model.clone())
        }
        _ => Err(invalid_response()),
    }
}

fn text(value: &Value) -> Result<&str, RunError> {
    value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(invalid_response)
}

fn has_calls(message: &Value) -> bool {
    message
        .get("function_call")
        .is_some_and(|value| !value.is_null())
        || message.get("tool_calls").is_some_and(|value| {
            !value.is_null() && value.as_array().is_none_or(|items| !items.is_empty())
        })
        || message
            .get("refusal")
            .is_some_and(|value| !value.is_null() && value.as_str() != Some(""))
}

fn decode(config: &AiProviderConfig, value: &Value) -> Result<ProviderAnswer, RunError> {
    let content = match config.provider {
        AiProvider::Openai | AiProvider::OpenaiCompatible => {
            let choices = value["choices"]
                .as_array()
                .filter(|choices| choices.len() == 1)
                .ok_or_else(invalid_response)?;
            let choice = &choices[0];
            let message = &choice["message"];
            if choice["finish_reason"] != "stop"
                || has_calls(message)
                || message["role"] != "assistant"
            {
                return Err(invalid_response());
            }
            text(&message["content"])?.to_owned()
        }
        AiProvider::Anthropic => {
            if value["stop_reason"] != "end_turn"
                || value["role"] != "assistant"
                || value
                    .pointer("/stop_details/type")
                    .is_some_and(|kind| kind == "refusal")
            {
                return Err(invalid_response());
            }
            let blocks = value["content"]
                .as_array()
                .filter(|blocks| !blocks.is_empty())
                .ok_or_else(invalid_response)?;
            let mut combined = String::new();
            for block in blocks {
                match block["type"].as_str() {
                    Some("text") => combined.push_str(text(&block["text"])?),
                    Some("thinking" | "redacted_thinking") => {}
                    _ => return Err(invalid_response()),
                }
            }
            if combined.trim().is_empty() {
                return Err(invalid_response());
            }
            combined
        }
        AiProvider::Gemini => {
            if value.get("promptFeedback").is_some_and(|feedback| {
                feedback.get("blockReason").is_some_and(|reason| {
                    !reason.is_null() && reason.as_str() != Some("BLOCK_REASON_UNSPECIFIED")
                })
            }) {
                return Err(invalid_response());
            }
            let candidates = value["candidates"]
                .as_array()
                .filter(|candidates| candidates.len() == 1)
                .ok_or_else(invalid_response)?;
            let candidate = &candidates[0];
            if candidate["finishReason"] != "STOP"
                || candidate["content"]["role"] != "model"
                || candidate["safetyRatings"]
                    .as_array()
                    .is_some_and(|ratings| ratings.iter().any(|rating| rating["blocked"] == true))
            {
                return Err(invalid_response());
            }
            let parts = candidate["content"]["parts"]
                .as_array()
                .filter(|parts| !parts.is_empty())
                .ok_or_else(invalid_response)?;
            let mut combined = String::new();
            for part in parts {
                if part.as_object().is_none_or(|fields| {
                    fields
                        .keys()
                        .any(|key| !matches!(key.as_str(), "text" | "thought" | "thoughtSignature"))
                }) {
                    return Err(invalid_response());
                }
                if part
                    .get("thought")
                    .is_some_and(|thought| !thought.is_boolean())
                {
                    return Err(invalid_response());
                }
                if part["thought"] != true {
                    combined.push_str(text(&part["text"])?);
                }
            }
            if combined.trim().is_empty() {
                return Err(invalid_response());
            }
            combined
        }
        AiProvider::Ollama => {
            let message = &value["message"];
            if value["done"] != true
                || value["done_reason"] != "stop"
                || message["role"] != "assistant"
                || has_calls(message)
            {
                return Err(invalid_response());
            }
            text(&message["content"])?.to_owned()
        }
    };
    Ok(ProviderAnswer {
        content,
        model: optional_model(value, &config.model)?,
    })
}

pub(super) struct ProviderAnswer {
    pub content: String,
    pub model: String,
}

pub(super) async fn request(
    client: &Client,
    config: &AiProviderConfig,
    key: Option<&str>,
    system: &str,
    content: &str,
) -> Result<ProviderAnswer, RunError> {
    let unavailable = || {
        failure(
            "AI_PROVIDER_UNAVAILABLE",
            "The provider request failed. Check the selected endpoint, key, model, and service availability, then review and submit again.",
        )
    };
    let mut response = build_request(client, config, key, system, content)?
        .send()
        .await
        .map_err(|_| unavailable())?;
    if !response.status().is_success() {
        return Err(unavailable());
    }
    if response
        .content_length()
        .is_some_and(|bytes| bytes > MAX_RESPONSE_BYTES as u64)
    {
        return Err(failure(
            "AI_RESPONSE_TOO_LARGE",
            "The provider response exceeded 256 KiB.",
        ));
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or(8192)
            .min(MAX_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(failure(
                "AI_RESPONSE_TOO_LARGE",
                "The provider response exceeded 256 KiB.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| invalid_response())?;
    decode(config, &value)
}

#[cfg(test)]
mod tests;

impl std::fmt::Debug for ProviderAnswer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProviderAnswer([redacted])")
    }
}

impl std::fmt::Debug for AiProviderConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AiProviderConfig([redacted])")
    }
}

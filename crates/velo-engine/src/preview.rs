//! Bounded single-request inspection using the load engine transport.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{NameValue, RunConfig, RunError};

/// A single logical response; never inserted into load-test history.
/// Body capture is limited to 64 KiB and headers to 16 KiB before redaction.
/// Debug formatting omits response content.
///
/// # Examples
/// ```
/// # fn example(response: velo_engine::PreviewResponse) {
/// assert!(response.elapsed_ms >= 0.0);
/// # }
/// ```
#[derive(Clone, Default, Deserialize, Serialize, TS)]
pub struct PreviewResponse {
    /// Assertion outcomes in configuration order; expectations and actual values are omitted.
    #[ts(type = "CheckOutcome[]")]
    pub checks: Vec<crate::CheckOutcome>,
    /// Final HTTP status, including non-success responses.
    pub status: u16,
    /// Elapsed time including redirects and bounded body consumption.
    pub elapsed_ms: f64,
    /// Ordered response headers, with sensitive values masked.
    #[ts(type = "NameValue[]")]
    pub headers: Vec<NameValue>,
    /// Lossy UTF-8 body, with known request values masked.
    pub body: String,
    /// Bytes received across response hops before bounded capture stopped; a lower bound when truncated.
    pub received_bytes: u64,
    /// Maximum retained body bytes, independent of the assertion inspection limit.
    pub body_limit: u32,
    /// Body representation; non-text and unsupported encodings are withheld.
    pub body_format: BodyFormat,
    /// True for live capture or opted-in history; false when retention omitted response content.
    pub content_retained: bool,
    /// Whether body or header capture exceeded its limit.
    pub truncated: bool,
    /// True when response content has been masked.
    pub redacted: bool,
}

/// Interpretation of a bounded response body; content is always rendered as data.
///
/// # Examples
/// ```
/// let _format = velo_engine::BodyFormat::Text;
/// ```
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BodyFormat {
    /// UTF-8 text; the caller may format valid JSON.
    #[default]
    Text,
    /// A binary content type; the body is withheld.
    Binary,
    /// Compressed or otherwise encoded bytes; the body is withheld.
    UnsupportedEncoding,
    /// Invalid UTF-8; lossy replacement would misrepresent the response.
    InvalidUtf8,
}

impl std::fmt::Debug for PreviewResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreviewResponse")
            .field("status", &self.status)
            .field("truncated", &self.truncated)
            .field("redacted", &self.redacted)
            .finish_non_exhaustive()
    }
}

/// Sends one logical request using engine validation, redirects, TLS and upload behavior.
/// The desktop caller must enforce authorization and admission before calling.
///
/// # Arguments
/// * `config` - Fully resolved request and valid workload/budget settings.
/// * `secrets` - Resolved secret values to mask, including values embedded inside request fields.
///
/// # Returns
/// Bounded response data with sensitive response headers and known request values masked.
/// Truncated bodies are withheld for explicit authentication or resolved secret references,
/// so a credential cut at the capture boundary cannot escape masking.
/// Dropping the future cancels upload producers; normal completion waits for their cleanup.
///
/// # Errors
/// Returns engine validation errors before network access, `RUN_PREPARATION_FAILED`, `PREVIEW_TIMEOUT`, or
/// `PREVIEW_TRANSPORT_FAILED`, or `PREVIEW_SCENARIO_UNSUPPORTED` for a scenario plan.
/// Infrastructure error messages and request data are omitted.
///
/// # Examples
/// ```no_run
/// # async fn example() -> Result<(), velo_engine::RunError> {
/// let response = velo_engine::preview_request(velo_engine::RunConfig {
///     url: "https://example.com/".into(), ..Default::default()
/// }, &[]).await?;
/// assert!(response.status >= 100);
/// # Ok(()) }
/// ```
pub async fn preview_request(
    mut config: RunConfig,
    secrets: &[String],
) -> Result<PreviewResponse, RunError> {
    let data = config.data.take();
    let request = serde_json::to_value(&config)
        .map_err(|_| RunError::new("INVALID_RUN_CONFIG", "Request could not be prepared.", None))?;
    config.data = data;
    if config.scenarios.is_some() {
        return Err(crate::RunError::new(
            "PREVIEW_SCENARIO_UNSUPPORTED",
            "Review and run the scenario to execute its ordered steps.",
            Some("scenarios"),
        ));
    }
    let validated = tokio::task::spawn_blocking(move || crate::config::validate(config))
        .await
        .map_err(|error| {
            if error.is_panic() {
                std::panic::resume_unwind(error.into_panic());
            }
            RunError::new(
                "RUN_PREPARATION_FAILED",
                "Request preparation could not complete.",
                None,
            )
        })??;
    let response =
        crate::transport::preview(&validated)
            .await
            .map_err(|failure| match failure {
                crate::transport::Failure::Timeout => RunError::new(
                    "PREVIEW_TIMEOUT",
                    "The request exceeded its configured timeout.",
                    Some("timeout_ms"),
                ),
                _ => RunError::new(
                    "PREVIEW_TRANSPORT_FAILED",
                    "The request failed. Check connectivity, redirects, TLS, and upload bindings.",
                    None,
                ),
            })?;
    redact_response(response, &validated, &request, secrets)
}

pub(crate) fn redact_response(
    mut response: PreviewResponse,
    validated: &crate::config::ValidatedConfig,
    request: &serde_json::Value,
    secrets: &[String],
) -> Result<PreviewResponse, RunError> {
    redact_response_fields(&mut response, &[]);
    let mut values = Vec::with_capacity(secrets.len());
    for secret in secrets {
        collect_values(&serde_json::Value::String(secret.clone()), &mut values);
    }
    for (name, value) in validated.request.headers() {
        if let Ok(value) = value.to_str()
            && !value.is_empty()
        {
            values.push(value.to_owned());
            if matches!(
                name,
                &reqwest::header::AUTHORIZATION | &reqwest::header::PROXY_AUTHORIZATION
            ) && let Some((scheme, credentials)) = value.split_once(' ')
                && scheme.eq_ignore_ascii_case("basic")
                && !credentials.trim().is_empty()
            {
                collect_values(
                    &serde_json::Value::String(credentials.trim().to_owned()),
                    &mut values,
                );
            }
        }
    }
    collect_values(request, &mut values);
    let explicit_auth = request
        .pointer("/auth/kind")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|kind| kind != "none")
        || validated.request.headers().keys().any(|name| {
            matches!(
                name,
                &reqwest::header::AUTHORIZATION
                    | &reqwest::header::PROXY_AUTHORIZATION
                    | &reqwest::header::COOKIE
            )
        });
    if response.truncated && (!secrets.is_empty() || explicit_auth) {
        response.body = "[Response body withheld: truncation may expose a partial secret.]".into();
        response.redacted = true;
    }
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values.dedup();
    let matcher = aho_corasick::AhoCorasickBuilder::new()
        .match_kind(aho_corasick::MatchKind::LeftmostLongest)
        .build(&values)
        .map_err(|_| {
            RunError::new(
                "PREVIEW_REDACTION_FAILED",
                "Response could not be safely redacted.",
                None,
            )
        })?;
    let replacements = vec!["[redacted]"; values.len()];
    for pair in &mut response.headers {
        let sensitive_header = matches!(
            pair.name.to_ascii_lowercase().as_str(),
            "set-cookie"
                | "authorization"
                | "proxy-authorization"
                | "www-authenticate"
                | "location"
        ) || ["token", "secret", "password", "api-key", "api_key"]
            .iter()
            .any(|name| pair.name.to_ascii_lowercase().contains(name));
        redact(
            &mut pair.name,
            &matcher,
            &replacements,
            &mut response.redacted,
        );
        if sensitive_header {
            pair.value = "[redacted]".into();
            response.redacted = true;
        } else {
            redact(
                &mut pair.value,
                &matcher,
                &replacements,
                &mut response.redacted,
            );
        }
    }
    redact(
        &mut response.body,
        &matcher,
        &replacements,
        &mut response.redacted,
    );
    if response.body.len() > 65_536 {
        let mut boundary = 65_536;
        while !response.body.is_char_boundary(boundary) {
            boundary -= 1;
        }
        response.body.truncate(boundary);
        response.truncated = true;
    }
    Ok(response)
}

fn collect_values(value: &serde_json::Value, values: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) if !text.is_empty() => {
            values.push(text.clone());
            values.push(
                percent_encoding::utf8_percent_encode(text, percent_encoding::NON_ALPHANUMERIC)
                    .to_string(),
            );
            if let Ok(embedded) = serde_json::from_str::<serde_json::Value>(text)
                && (embedded.is_array() || embedded.is_object())
            {
                collect_values(&embedded, values);
            }
            values.push(
                serde_json::to_string(text)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_owned(),
            );
            if let Ok(url) = reqwest::Url::parse(text) {
                for (_, value) in url.query_pairs() {
                    if !value.is_empty() {
                        values.push(value.into_owned());
                    }
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_values(item, values);
            }
        }
        serde_json::Value::Object(items) => {
            for value in items.values() {
                collect_values(value, values);
            }
        }
        _ => {}
    }
}

fn redact(
    text: &mut String,
    matcher: &aho_corasick::AhoCorasick,
    replacements: &[&str],
    redacted: &mut bool,
) {
    if matcher.is_match(text.as_str()) {
        *text = matcher.replace_all(text.as_str(), replacements);
        *redacted = true;
    }
}

/// Masks configured JSON response fields before display, persistence, or export.
///
/// # Arguments
/// * `response` - Bounded response whose checks and extractions already ran on original bytes.
/// * `pointers` - Validated RFC 6901 pointers from the collection's sensitive field settings.
///
/// # Returns
/// No value. Truncated, invalid, or non-JSON bodies are withheld when pointers are configured.
/// Common credential field names are masked in JSON even without configured pointers.
///
/// # Examples
/// ```
/// let mut response = velo_engine::PreviewResponse::default();
/// response.body = r#"{"token":"private","ok":true}"#.into();
/// velo_engine::redact_response_fields(&mut response, &[]);
/// assert!(!response.body.contains("private"));
/// ```
pub fn redact_response_fields(response: &mut PreviewResponse, pointers: &[String]) {
    if response.body.is_empty() {
        return;
    }
    let json_content = response.headers.iter().any(|header| {
        header.name.eq_ignore_ascii_case("content-type")
            && header.value.to_ascii_lowercase().contains("json")
    }) || response.body.trim_start().starts_with(['{', '[']);
    if response.truncated && (!pointers.is_empty() || json_content) {
        response.body =
            "[Response body withheld: sensitive field masking requires complete JSON.]".into();
        response.redacted = true;
        return;
    }
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&response.body) else {
        if !pointers.is_empty() {
            response.body =
                "[Response body withheld: sensitive field masking requires valid JSON.]".into();
            response.redacted = true;
        }
        return;
    };
    mask_credentials(&mut value, &mut response.redacted);
    for pointer in pointers {
        if let Some(field) = value.pointer_mut(pointer) {
            *field = serde_json::Value::String("[redacted]".into());
            response.redacted = true;
        }
    }
    if response.redacted {
        response.body =
            serde_json::to_string(&value).unwrap_or_else(|_| "[Response withheld]".into());
        if response.body.len() > response.body_limit as usize {
            response.body =
                "[Response body withheld: masked JSON exceeds the capture limit.]".into();
            response.truncated = true;
        }
    }
}

fn mask_credentials(value: &mut serde_json::Value, redacted: &mut bool) {
    match value {
        serde_json::Value::Object(fields) => {
            for (name, value) in fields {
                let name = name.to_ascii_lowercase().replace(['-', '_'], "");
                if [
                    "password",
                    "secret",
                    "token",
                    "authorization",
                    "cookie",
                    "apikey",
                ]
                .iter()
                .any(|part| name.contains(part))
                {
                    *value = serde_json::Value::String("[redacted]".into());
                    *redacted = true;
                } else {
                    mask_credentials(value, redacted);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                mask_credentials(item, redacted);
            }
        }
        _ => {}
    }
}

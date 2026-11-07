//! Bounded single-request inspection using the load engine transport.

use serde::Serialize;
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
#[derive(Default, Serialize, TS)]
pub struct PreviewResponse {
    /// Final HTTP status, including non-success responses.
    pub status: u16,
    /// Elapsed time including redirects and bounded body consumption.
    pub elapsed_ms: f64,
    /// Ordered response headers, with sensitive values masked.
    #[ts(type = "NameValue[]")]
    pub headers: Vec<NameValue>,
    /// Lossy UTF-8 body, with known request values masked.
    pub body: String,
    /// Whether body or header capture exceeded its limit.
    pub truncated: bool,
    /// True when response content has been masked.
    pub redacted: bool,
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
/// Truncated bodies are withheld when secret references were resolved, so partial secrets cannot leak.
/// Dropping the future cancels upload producers; normal completion waits for their cleanup.
///
/// # Errors
/// Returns engine validation errors before network access, `RUN_PREPARATION_FAILED`, `PREVIEW_TIMEOUT`, or
/// `PREVIEW_TRANSPORT_FAILED`. Infrastructure error messages and request data are omitted.
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
    let mut response =
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
    let mut values = Vec::with_capacity(secrets.len());
    for secret in secrets {
        collect_values(&serde_json::Value::String(secret.clone()), &mut values);
    }
    for value in validated.request.headers().values() {
        if let Ok(value) = value.to_str()
            && !value.is_empty()
        {
            values.push(value.to_owned());
        }
    }
    collect_values(&request, &mut values);
    if response.truncated && !secrets.is_empty() {
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
        );
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

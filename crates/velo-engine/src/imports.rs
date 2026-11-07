//! Offline request conversion. Inputs are data and are never executed or fetched.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::{NameValue, RunConfig, RunError};

mod curl;
mod openapi;
mod postman;

/// Supported offline import sources. OpenAPI and Postman inputs use JSON.
///
/// # Examples
/// ```
/// let format = velo_engine::ImportFormat::Curl;
/// assert_eq!(serde_json::to_string(&format)?, "\"curl\"");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    /// A shell-quoted cURL command, parsed without a shell.
    Curl,
    /// OpenAPI 3 JSON, one candidate per supported operation.
    Openapi,
    /// Postman v2 collection JSON, recursively listing requests.
    Postman,
}

/// An editable request candidate and all conversion limitations.
/// Warnings must be reviewed before applying the candidate.
/// Debug formatting omits imported source content.
///
/// # Examples
/// ```
/// let requests = velo_engine::import_requests(velo_engine::ImportFormat::Curl, "curl https://example.com")?;
/// assert_eq!(requests.len(), 1);
/// # Ok::<(), velo_engine::RunError>(())
/// ```
#[derive(Serialize, TS)]
pub struct ImportedRequest {
    /// Display name from the source; may contain private information.
    pub name: String,
    /// Converted request with default workload and budgets.
    pub config: RunConfig,
    /// Unsupported fields, security, scripts, or unresolved references.
    #[ts(type = "string[]")]
    pub warnings: Vec<String>,
}

impl std::fmt::Debug for ImportedRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ImportedRequest([redacted])")
    }
}

fn invalid(message: &str) -> RunError {
    RunError::new("REQUEST_IMPORT_FAILED", message, Some("source"))
}

fn unsupported(value: &Value, supported: &[&str], location: &str, warnings: &mut Vec<String>) {
    if let Some(fields) = value.as_object() {
        for key in fields.keys() {
            if !supported.contains(&key.as_str()) {
                if warnings.len() >= 64 {
                    return;
                }
                let key: String = key.chars().take(64).collect();
                warnings.push(format!(
                    "Unsupported {location} field: {key}. Review the source before running."
                ));
            }
        }
    }
}

fn method(value: &str) -> Result<crate::RequestMethod, RunError> {
    serde_json::from_value(Value::String(value.into()))
        .map_err(|_| invalid("Use a supported HTTP method."))
}

fn text(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}

fn pairs(value: Option<&Value>, warnings: &mut Vec<String>) -> Vec<NameValue> {
    let mut pairs = Vec::new();
    if let Some(Value::Array(items)) = value {
        for item in items {
            unsupported(
                item,
                &["key", "value", "disabled", "description", "type"],
                "pair",
                warnings,
            );
            if item.get("disabled").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            if item.get("value").is_some_and(|value| !value.is_string()) {
                warnings.push("Non-string pair value requires review.".into());
            }
            pairs.push(NameValue {
                name: text(item, "key"),
                value: text(item, "value"),
            });
        }
    }
    pairs
}

/// Converts supported requests offline, reporting fields that need manual review.
/// No shell commands, collection scripts, external references, or network requests execute.
///
/// # Arguments
/// * `format` - Explicit source format; OpenAPI/Postman accept JSON only.
/// * `source` - Source text, bounded to 8 MiB.
///
/// # Returns
/// Request candidates with conversion warnings; selecting and applying one is a separate action.
///
/// # Errors
/// Returns `REQUEST_IMPORT_FAILED` for malformed/oversized input, unsupported cURL flags,
/// unsupported methods or collection limits. Messages omit source values.
///
/// # Examples
/// ```
/// let requests = velo_engine::import_requests(velo_engine::ImportFormat::Curl,
///     "curl -H 'Accept: application/json' https://example.com")?;
/// assert_eq!(requests[0].config.headers.len(), 1);
/// # Ok::<(), velo_engine::RunError>(())
/// ```
pub fn import_requests(
    format: ImportFormat,
    source: &str,
) -> Result<Vec<ImportedRequest>, RunError> {
    if source.len() > 8_388_608 {
        return Err(invalid("Import source exceeds 8 MiB."));
    }
    let result = match format {
        ImportFormat::Curl => curl::convert(source)?,
        ImportFormat::Openapi => openapi::convert(
            &serde_json::from_str(source)
                .map_err(|_| invalid("Enter valid OpenAPI JSON. YAML is not supported."))?,
        )?,
        ImportFormat::Postman => {
            let value: Value = serde_json::from_str(source)
                .map_err(|_| invalid("Enter valid Postman collection JSON."))?;
            let mut warnings = Vec::new();
            unsupported(
                &value,
                &["info", "item"],
                "collection (variables/authentication/scripts require review)",
                &mut warnings,
            );
            let items = value["item"]
                .as_array()
                .ok_or_else(|| invalid("Select a Postman v2 collection with an item array."))?;
            let mut output = Vec::new();
            postman::convert(items, &warnings, &mut output, 0)?;
            output
        }
    };
    if result
        .iter()
        .any(|candidate| candidate.warnings.len() >= 64)
    {
        return Err(invalid(
            "The source has too many unsupported fields to review safely. Simplify it to fewer than 64 warnings per request.",
        ));
    }
    if result.is_empty() {
        return Err(invalid("No supported requests were found."));
    }
    Ok(result)
}

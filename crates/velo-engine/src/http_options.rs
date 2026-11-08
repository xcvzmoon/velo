use std::{path::Path, time::SystemTime};

use reqwest::{RequestBuilder, header::HeaderValue};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{NameValue, RequestBody, RunError};

pub(crate) const UPLOAD_BUFFER_BYTES: usize = 16 * 1024;

/// Explicit request credentials, retained in memory and omitted from reports.
/// Basic authentication is encoded, not encrypted; use HTTPS for credential protection.
///
/// # Examples
/// ```
/// let auth = velo_engine::RequestAuth::Bearer { token: "token".into() };
/// assert_eq!(serde_json::to_value(auth)?["kind"], "bearer");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Default, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RequestAuth {
    /// No dedicated credentials; a custom Authorization header is still supported.
    #[default]
    None,
    /// A nonempty RFC 6750 token, without whitespace or control characters.
    Bearer {
        /// Secret token, never returned in run snapshots.
        token: String,
    },
    /// A named API key, placed in one header or one query parameter.
    ApiKey {
        /// Header or query parameter name; must not conflict with an explicit request field.
        /// Header placement excludes framing, Authorization, Proxy-Authorization, and Cookie headers.
        name: String,
        /// Secret value, omitted from reports and Debug output.
        value: String,
        /// Explicit placement; query values can appear in server access logs.
        placement: ApiKeyPlacement,
    },
    /// Basic credentials. Username cannot contain a colon or control characters.
    Basic {
        /// User identifier; may be empty.
        username: String,
        /// Secret password; may be empty, but cannot contain control characters.
        password: String,
    },
}

/// Placement of an API key in a shared HTTP request.
///
/// # Examples
/// ```
/// let placement = velo_engine::ApiKeyPlacement::Header;
/// assert_eq!(serde_json::to_value(placement)?, "header");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyPlacement {
    /// A single named HTTP header.
    Header,
    /// A single URL query parameter.
    Query,
}

impl std::fmt::Debug for RequestAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RequestAuth([redacted])")
    }
}

/// Connection protocol policy. Auto negotiates HTTP/1.1 or HTTP/2 over TLS.
/// HTTP/2 uses prior knowledge for cleartext servers and requires h2 over TLS.
///
/// # Examples
/// ```
/// assert_eq!(serde_json::to_value(velo_engine::HttpProtocol::Http1)?, "http1");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum HttpProtocol {
    /// Negotiate normally; cleartext defaults to HTTP/1.1.
    #[default]
    Auto,
    /// Require HTTP/1.1.
    Http1,
    /// Require HTTP/2 without falling back to HTTP/1.1.
    Http2,
}

/// One multipart field. File contents are streamed afresh for each logical attempt.
/// Files must be regular, readable files with absolute paths and remain unchanged
/// throughout the run. Paths and field values are omitted from run reports.
/// Debug formatting omits file metadata and field contents.
///
/// # Examples
/// ```
/// let part = velo_engine::MultipartPart::Text { name: "tag".into(), value: "one".into() };
/// assert_eq!(serde_json::to_value(part)?["kind"], "text");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MultipartPart {
    /// UTF-8 field; duplicate names preserve order.
    Text {
        /// Multipart field name without control characters.
        name: String,
        /// UTF-8 content.
        value: String,
    },
    /// Streamed file with an explicit public upload filename and MIME type.
    File {
        /// Multipart field name without control characters.
        name: String,
        /// Absolute local file path, never used as the transmitted filename.
        path: String,
        /// Transmitted filename; no path separators or control characters.
        filename: String,
        /// Valid MIME type; defaults in the editor to application/octet-stream.
        content_type: String,
    },
}

impl std::fmt::Debug for MultipartPart {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Text { .. } => "MultipartPart::Text([redacted])",
            Self::File { .. } => "MultipartPart::File([redacted])",
        })
    }
}

#[derive(Clone)]
pub(crate) enum PreparedPart {
    Text(NameValue),
    File {
        name: String,
        path: String,
        filename: String,
        content_type: String,
        length: u64,
        modified: Option<SystemTime>,
    },
}

pub(crate) fn credential_field(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace(['-', '_'], "");
    matches!(
        normalized.as_str(),
        "authorization"
            | "proxyauthorization"
            | "cookie"
            | "apikey"
            | "xapikey"
            | "accesstoken"
            | "xaccesstoken"
            | "authtoken"
            | "xauthtoken"
            | "clientsecret"
            | "password"
            | "secret"
            | "token"
            | "xtoken"
            | "sessionid"
    )
}

pub(crate) fn auth_size(auth: &RequestAuth) -> usize {
    match auth {
        RequestAuth::None => 0,
        RequestAuth::Bearer { token } => token.len(),
        RequestAuth::ApiKey { name, value, .. } => name.len().saturating_add(value.len()),
        RequestAuth::Basic { username, password } => username.len().saturating_add(password.len()),
    }
}

pub(crate) fn body_size(body: &RequestBody) -> usize {
    match body {
        RequestBody::None => 0,
        RequestBody::Text(value) | RequestBody::Json(value) => value.len(),
        RequestBody::Form(pairs) => pairs.iter().fold(0_usize, |size, pair| {
            size.saturating_add(std::mem::size_of::<NameValue>())
                .saturating_add(pair.name.len())
                .saturating_add(pair.value.len())
        }),
        RequestBody::Multipart(parts) => parts.iter().fold(0_usize, |size, part| {
            let bytes = match part {
                MultipartPart::Text { name, value } => name.len().saturating_add(value.len()),
                MultipartPart::File {
                    name,
                    path,
                    filename,
                    content_type,
                } => name
                    .len()
                    .saturating_add(path.len())
                    .saturating_add(filename.len())
                    .saturating_add(content_type.len()),
            };
            size.saturating_add(std::mem::size_of::<PreparedPart>())
                .saturating_add(bytes)
        }),
    }
}

pub(crate) fn authenticate(
    request: RequestBuilder,
    auth: &RequestAuth,
) -> Result<RequestBuilder, RunError> {
    let invalid = || {
        RunError::new(
            "INVALID_RUN_CONFIG",
            "Enter valid credentials without control characters; Basic usernames cannot contain a colon.",
            Some("auth"),
        )
    };
    match auth {
        RequestAuth::None => Ok(request),
        RequestAuth::ApiKey {
            name,
            value,
            placement,
        } => {
            if name.is_empty()
                || value.is_empty()
                || name.chars().chain(value.chars()).any(char::is_control)
            {
                return Err(invalid());
            }
            match placement {
                ApiKeyPlacement::Header => {
                    let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                        .map_err(|_| invalid())?;
                    if matches!(
                        name,
                        reqwest::header::HOST
                            | reqwest::header::CONTENT_LENGTH
                            | reqwest::header::TRANSFER_ENCODING
                            | reqwest::header::AUTHORIZATION
                            | reqwest::header::PROXY_AUTHORIZATION
                            | reqwest::header::COOKIE
                    ) {
                        return Err(RunError::new(
                            "INVALID_RUN_CONFIG",
                            "Use a dedicated authentication control or an explicit credential header instead of an API key in a reserved HTTP header.",
                            Some("auth"),
                        ));
                    }
                    let mut value = HeaderValue::from_str(value).map_err(|_| invalid())?;
                    value.set_sensitive(true);
                    Ok(request.header(name, value))
                }
                ApiKeyPlacement::Query => Ok(request.query(&[(name, value)])),
            }
        }
        RequestAuth::Bearer { token } => {
            let base = token.trim_end_matches('=');
            if base.is_empty()
                || !base
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/".contains(&byte))
                || HeaderValue::from_str(&format!("Bearer {token}")).is_err()
            {
                return Err(invalid());
            }
            Ok(request.bearer_auth(token))
        }
        RequestAuth::Basic { username, password } => {
            if username.contains(':')
                || username
                    .chars()
                    .chain(password.chars())
                    .any(char::is_control)
            {
                return Err(invalid());
            }
            Ok(request.basic_auth(username, Some(password)))
        }
    }
}

pub(crate) fn prepare_parts_cached(
    parts: Vec<MultipartPart>,
    cached: Option<&[PreparedPart]>,
) -> Result<Vec<PreparedPart>, RunError> {
    let invalid = || {
        RunError::new(
            "INVALID_RUN_CONFIG",
            "Enter valid multipart names, filenames, and MIME types.",
            Some("body"),
        )
    };
    let file_error = || {
        RunError::new(
            "UPLOAD_FILE_UNAVAILABLE",
            "Choose a readable regular file with an absolute path.",
            Some("body"),
        )
    };
    let mut prepared = Vec::with_capacity(parts.len());
    for (index, part) in parts.into_iter().enumerate() {
        let name = match &part {
            MultipartPart::Text { name, .. } | MultipartPart::File { name, .. } => name,
        };
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(invalid());
        }
        prepared.push(match part {
            MultipartPart::Text { name, value } => PreparedPart::Text(NameValue { name, value }),
            MultipartPart::File {
                name,
                path,
                filename,
                content_type,
            } => {
                if filename.is_empty()
                    || filename.contains(['/', '\\'])
                    || filename.chars().any(char::is_control)
                    || reqwest::multipart::Part::text("")
                        .mime_str(&content_type)
                        .is_err()
                {
                    return Err(invalid());
                }
                if let Some(cached) = cached {
                    let Some(PreparedPart::File {
                        path: original,
                        length,
                        modified,
                        ..
                    }) = cached.get(index)
                    else {
                        return Err(file_error());
                    };
                    if original != &path {
                        return Err(file_error());
                    }
                    prepared.push(PreparedPart::File {
                        name,
                        path,
                        filename,
                        content_type,
                        length: *length,
                        modified: *modified,
                    });
                    continue;
                }
                if !Path::new(&path).is_absolute()
                    || !std::fs::metadata(&path)
                        .map_err(|_| file_error())?
                        .is_file()
                {
                    return Err(file_error());
                }
                let file = std::fs::File::open(&path).map_err(|_| file_error())?;
                let metadata = file.metadata().map_err(|_| file_error())?;
                if !metadata.is_file() {
                    return Err(file_error());
                }
                PreparedPart::File {
                    name,
                    path,
                    filename,
                    content_type,
                    length: metadata.len(),
                    modified: metadata.modified().ok(),
                }
            }
        });
    }
    Ok(prepared)
}

pub(crate) fn upload_buffer_bytes(parts: Option<&[PreparedPart]>) -> usize {
    parts.map_or(0, |parts| {
        parts
            .iter()
            .filter(|part| matches!(part, PreparedPart::File { .. }))
            .count()
            .saturating_mul(UPLOAD_BUFFER_BYTES * 2)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RequestMethod, RunConfig, RunManager};

    #[test]
    fn generated_and_explicit_authentication_headers_are_sensitive() {
        let client = reqwest::Client::new();
        let request = authenticate(
            client.get("https://example.com"),
            &RequestAuth::ApiKey {
                name: "x-api-key".into(),
                value: "private-key".into(),
                placement: ApiKeyPlacement::Header,
            },
        )
        .unwrap()
        .build()
        .unwrap();
        assert!(request.headers().get("x-api-key").unwrap().is_sensitive());
        assert!(!format!("{request:?}").contains("private-key"));
        for name in ["Authorization", "Proxy-Authorization", "Cookie"] {
            let config = crate::config::validate(RunConfig {
                url: "https://example.com".into(),
                headers: vec![NameValue {
                    name: name.into(),
                    value: "private-value".into(),
                }],
                ..Default::default()
            })
            .unwrap();
            assert!(config.request.headers().get(name).unwrap().is_sensitive());
            assert!(!format!("{:?}", config.request).contains("private-value"));
        }
    }

    #[test]
    fn rejects_invalid_auth_and_conflicts_without_disclosing_credentials() {
        for auth in [
            RequestAuth::Bearer {
                token: "private token".into(),
            },
            RequestAuth::Bearer {
                token: "===".into(),
            },
            RequestAuth::Basic {
                username: "private:user".into(),
                password: "private".into(),
            },
            RequestAuth::Basic {
                username: "user".into(),
                password: "private\n".into(),
            },
        ] {
            let error = RunManager::prepare(RunConfig {
                url: "http://127.0.0.1/".into(),
                auth,
                ..Default::default()
            })
            .unwrap_err();
            assert_eq!(error.code, "INVALID_RUN_CONFIG");
            assert_eq!(error.field.as_deref(), Some("auth"));
            assert!(!serde_json::to_string(&error).unwrap().contains("private"));
        }
        let auth = RequestAuth::Bearer {
            token: "private-token".into(),
        };
        assert!(!format!("{auth:?}").contains("private-token"));
        let error = RunManager::prepare(RunConfig {
            url: "http://127.0.0.1/".into(),
            auth,
            headers: vec![NameValue {
                name: "Authorization".into(),
                value: "private-token".into(),
            }],
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(error.field.as_deref(), Some("auth"));
        assert!(!error.to_string().contains("private-token"));
    }

    #[test]
    fn multipart_validation_rejects_unavailable_files_invalid_metadata_and_conflicting_headers() {
        let directory = tempfile::tempdir().unwrap();
        for (path, filename, content_type, code) in [
            (
                directory.path().to_str().unwrap().to_owned(),
                "upload.txt",
                "text/plain",
                "UPLOAD_FILE_UNAVAILABLE",
            ),
            (
                directory
                    .path()
                    .join("private-missing-file")
                    .to_str()
                    .unwrap()
                    .to_owned(),
                "upload.txt",
                "text/plain",
                "UPLOAD_FILE_UNAVAILABLE",
            ),
            (
                "relative-file".into(),
                "upload.txt",
                "text/plain",
                "UPLOAD_FILE_UNAVAILABLE",
            ),
            (
                "/private-file".into(),
                "../private-file",
                "text/plain",
                "INVALID_RUN_CONFIG",
            ),
            (
                "/private-file".into(),
                "upload.txt",
                "invalid mime",
                "INVALID_RUN_CONFIG",
            ),
        ] {
            let error = RunManager::prepare(RunConfig {
                url: "http://127.0.0.1/".into(),
                method: RequestMethod::POST,
                body: RequestBody::Multipart(vec![MultipartPart::File {
                    name: "upload".into(),
                    path,
                    filename: filename.into(),
                    content_type: content_type.into(),
                }]),
                ..Default::default()
            })
            .unwrap_err();
            assert_eq!(error.code, code);
            assert!(!error.to_string().contains("private"));
        }
        let error = RunManager::prepare(RunConfig {
            url: "http://127.0.0.1/".into(),
            method: RequestMethod::POST,
            body: RequestBody::Multipart(vec![]),
            headers: vec![NameValue {
                name: "content-type".into(),
                value: "multipart/form-data".into(),
            }],
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(error.field.as_deref(), Some("headers"));
        for method in [RequestMethod::GET, RequestMethod::HEAD] {
            let error = RunManager::prepare(RunConfig {
                url: "http://127.0.0.1/".into(),
                method,
                body: RequestBody::Form(vec![]),
                ..Default::default()
            })
            .unwrap_err();
            assert_eq!(error.field.as_deref(), Some("body"));
        }
    }
}

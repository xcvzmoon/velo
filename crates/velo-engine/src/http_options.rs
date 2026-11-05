use reqwest::{RequestBuilder, header::HeaderValue};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::RunError;

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
    /// Basic credentials. Username cannot contain a colon or control characters.
    Basic {
        /// User identifier; may be empty.
        username: String,
        /// Secret password; may be empty, but cannot contain control characters.
        password: String,
    },
}

impl std::fmt::Debug for RequestAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RequestAuth([redacted])")
    }
}

pub(crate) fn auth_size(auth: &RequestAuth) -> usize {
    match auth {
        RequestAuth::None => 0,
        RequestAuth::Bearer { token } => token.len(),
        RequestAuth::Basic { username, password } => username.len().saturating_add(password.len()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NameValue, RunConfig, RunManager};
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
}

//! Explicit OAuth token acquisition with bounded loopback callbacks and OS credential storage.

use std::{sync::Arc, time::Duration};

use oauth2::{CsrfToken, PkceCodeChallenge};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex, oneshot},
};
use tokio_util::sync::CancellationToken;
use velo_engine::RunError;

use crate::{execution::DesktopRuns, history::History};

mod credentials;

const FLOW_TIMEOUT: Duration = Duration::from_secs(180);
const TOKEN_BYTES: usize = 65_536;
const CALLBACK_BYTES: usize = 8_192;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OAuthConfig {
    credential_id: String,
    grant: Grant,
    token_endpoint: String,
    authorization_endpoint: Option<String>,
    issuer: Option<String>,
    client_id: String,
    client_secret_name: Option<String>,
    scope: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Grant {
    ClientCredentials,
    AuthorizationCode,
}

#[derive(Serialize)]
pub(crate) struct OAuthPending {
    id: String,
    redirect_uri: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct OAuthToken {
    reference: String,
    expires_in: Option<u64>,
    refresh_available: bool,
}

#[derive(Deserialize, Serialize)]
struct StoredToken {
    access_token: String,
    refresh_token: Option<String>,
    token_endpoint: String,
    client_id: String,
    issuer: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

struct Flow {
    id: String,
    cancel: CancellationToken,
    result: Option<oneshot::Receiver<Result<OAuthToken, RunError>>>,
    task: Option<tauri::async_runtime::JoinHandle<Result<(), RunError>>>,
}

impl Drop for Flow {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[derive(Default)]
pub(crate) struct OAuthState(Mutex<Option<Flow>>);

fn failure(code: &str, message: &str) -> RunError {
    RunError {
        code: code.into(),
        message: message.into(),
        field: Some("oauth".into()),
    }
}

fn endpoint(source: &str) -> Result<Url, RunError> {
    if source.len() > 4_096 || source.chars().any(char::is_control) || source.trim() != source {
        return Err(failure(
            "OAUTH_INVALID_CONFIG",
            "OAuth endpoints must fit within 4 KiB.",
        ));
    }
    let url = Url::parse(source).map_err(|_| {
        failure(
            "OAUTH_INVALID_CONFIG",
            "Use an absolute HTTPS OAuth endpoint.",
        )
    })?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query_pairs().any(|(name, _)| reserved_parameter(&name))
    {
        return Err(failure(
            "OAUTH_INVALID_CONFIG",
            "OAuth endpoints require HTTPS without URL credentials, fragments, or conflicting OAuth query parameters.",
        ));
    }
    Ok(url)
}

fn reserved_parameter(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "response_type"
            | "grant_type"
            | "client_id"
            | "client_secret"
            | "redirect_uri"
            | "scope"
            | "state"
            | "code"
            | "code_verifier"
            | "code_challenge"
            | "code_challenge_method"
            | "access_token"
            | "refresh_token"
            | "authorization"
            | "secret"
            | "password"
            | "token"
            | "api_key"
            | "api-key"
    )
}

fn validate_issuer(source: &str) -> Result<(), RunError> {
    let issuer = endpoint(source)?;
    if issuer.query().is_some() {
        return Err(failure(
            "OAUTH_INVALID_CONFIG",
            "The issuer identifier must be HTTPS without a query or fragment.",
        ));
    }
    Ok(())
}

fn valid_scope(scope: &str) -> bool {
    scope.is_empty()
        || scope.split(' ').all(|token| {
            !token.is_empty()
                && token
                    .bytes()
                    .all(|byte| matches!(byte, b'!' | b'#'..=b'[' | b']'..=b'~'))
        })
}

fn validate(config: &OAuthConfig) -> Result<(), RunError> {
    if config.credential_id.is_empty()
        || config.credential_id.len() > 40
        || !config
            .credential_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        || config.client_id.is_empty()
        || config.client_id.len() > 4_096
        || config.scope.len() > 4_096
        || !valid_scope(&config.scope)
        || config
            .client_id
            .chars()
            .chain(config.scope.chars())
            .any(char::is_control)
    {
        return Err(failure(
            "OAUTH_INVALID_CONFIG",
            "Use a credential ID of 1 to 40 letters, digits, underscores or hyphens and bounded client and scope values.",
        ));
    }
    endpoint(&config.token_endpoint)?;
    if let Some(issuer) = &config.issuer {
        validate_issuer(issuer)?;
    }
    if matches!(config.grant, Grant::AuthorizationCode) {
        validate_issuer(config.issuer.as_deref().ok_or_else(|| failure("OAUTH_INVALID_CONFIG", "Set the provider's issuer identifier for PKCE. The provider must support RFC 9207 issuer responses."))?)?;
        endpoint(config.authorization_endpoint.as_deref().ok_or_else(|| {
            failure(
                "OAUTH_INVALID_CONFIG",
                "Set an authorization endpoint for PKCE.",
            )
        })?)?;
    }
    if matches!(config.grant, Grant::ClientCredentials) && config.client_secret_name.is_none() {
        return Err(failure(
            "OAUTH_INVALID_CONFIG",
            "Client credentials requires an OS secret reference for the client secret.",
        ));
    }
    if let Some(name) = &config.client_secret_name {
        crate::authoring::validate_secret_name(name)?;
    }
    Ok(())
}

fn entry(id: &str) -> Result<keyring::Entry, RunError> {
    keyring::Entry::new("app.velo.authoring", &format!("oauth.{id}")).map_err(|_| {
        failure(
            "OAUTH_CREDENTIAL_UNAVAILABLE",
            "The OS credential store is unavailable.",
        )
    })
}

pub(crate) fn read_access(name: &str) -> Result<String, RunError> {
    let id = name.strip_prefix("oauth.").ok_or_else(|| {
        failure(
            "OAUTH_CREDENTIAL_UNAVAILABLE",
            "Acquire the OAuth token again.",
        )
    })?;
    let source = entry(id)?.get_password().map_err(|_| {
        failure(
            "OAUTH_CREDENTIAL_UNAVAILABLE",
            "Acquire the OAuth token before sending the request.",
        )
    })?;
    if source.len() > TOKEN_BYTES {
        return Err(failure(
            "OAUTH_CREDENTIAL_UNAVAILABLE",
            "Acquire the OAuth token again.",
        ));
    }
    let stored: StoredToken = serde_json::from_str(&source).map_err(|_| {
        failure(
            "OAUTH_CREDENTIAL_UNAVAILABLE",
            "Acquire the OAuth token again.",
        )
    })?;
    Ok(stored.access_token)
}

async fn client_secret<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: &OAuthConfig,
) -> Result<Option<String>, RunError> {
    let Some(name) = config.client_secret_name.clone() else {
        return Ok::<_, RunError>(None);
    };
    app.state::<History>()
        .configuration(true, move |_| {
            crate::authoring::read_named_secret(&name).map(Some)
        })
        .await
}

async fn refresh_token<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: &OAuthConfig,
) -> Result<String, RunError> {
    let config = config.clone();
    app.state::<History>()
        .configuration(true, move |_| {
            let source = entry(&config.credential_id)?.get_password().map_err(|_| {
                failure(
                    "OAUTH_CREDENTIAL_UNAVAILABLE",
                    "Acquire a token before refreshing.",
                )
            })?;
            if source.len() > TOKEN_BYTES {
                return Err(failure(
                    "OAUTH_CREDENTIAL_UNAVAILABLE",
                    "Acquire a token again.",
                ));
            }
            let stored: StoredToken = serde_json::from_str(&source)
                .map_err(|_| failure("OAUTH_CREDENTIAL_UNAVAILABLE", "Acquire a token again."))?;
            validate_refresh(&stored, &config)?;
            stored.refresh_token.ok_or_else(|| {
                failure(
                    "OAUTH_REFRESH_UNAVAILABLE",
                    "The provider did not issue a refresh token. Acquire a new token.",
                )
            })
        })
        .await
}

fn validate_refresh(stored: &StoredToken, config: &OAuthConfig) -> Result<(), RunError> {
    if stored.token_endpoint != config.token_endpoint
        || stored.client_id != config.client_id
        || stored.issuer != config.issuer
    {
        return Err(failure(
            "OAUTH_CONFIG_CHANGED",
            "Acquire a new token after changing the endpoint, issuer, or client ID.",
        ));
    }
    Ok(())
}

fn authorization_request(
    config: &OAuthConfig,
    redirect: &str,
    csrf: &CsrfToken,
    challenge: &PkceCodeChallenge,
) -> Result<Url, RunError> {
    let mut url = endpoint(config.authorization_endpoint.as_deref().unwrap_or_default())?;
    url.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", &config.client_id),
        ("redirect_uri", redirect),
        ("state", csrf.secret()),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
    ]);
    if !config.scope.is_empty() {
        url.query_pairs_mut().append_pair("scope", &config.scope);
    }
    Ok(url)
}

fn token_request(
    client: &reqwest::Client,
    config: &OAuthConfig,
    secret: Option<String>,
    mut parameters: Vec<(&str, String)>,
) -> Result<reqwest::RequestBuilder, RunError> {
    let mut request = client
        .post(endpoint(&config.token_endpoint)?)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(secret) = secret {
        let id: String =
            url::form_urlencoded::byte_serialize(config.client_id.as_bytes()).collect();
        let secret: String = url::form_urlencoded::byte_serialize(secret.as_bytes()).collect();
        request = request.basic_auth(id, Some(secret));
    } else {
        parameters.push(("client_id", config.client_id.clone()));
    }
    Ok(request.form(&parameters))
}

async fn exchange(
    config: &OAuthConfig,
    secret: Option<String>,
    parameters: Vec<(&str, String)>,
) -> Result<TokenResponse, RunError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| {
            failure(
                "OAUTH_TRANSPORT_FAILED",
                "The OAuth client could not be initialized.",
            )
        })?;
    let request = token_request(&client, config, secret, parameters)?;
    let mut response = request.send().await.map_err(|_| {
        failure(
            "OAUTH_TRANSPORT_FAILED",
            "The token request failed. Check connectivity and TLS.",
        )
    })?;
    if !response.status().is_success() {
        return Err(failure(
            "OAUTH_TOKEN_REJECTED",
            "The provider rejected the token request. Check credentials, scopes, and grant settings.",
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        failure(
            "OAUTH_TRANSPORT_FAILED",
            "The token response could not be read.",
        )
    })? {
        if body.len().saturating_add(chunk.len()) > TOKEN_BYTES {
            return Err(failure(
                "OAUTH_RESPONSE_LIMIT",
                "The token response exceeded 64 KiB.",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    parse_token_response(&body)
}

fn parse_token_response(body: &[u8]) -> Result<TokenResponse, RunError> {
    if body.len() > TOKEN_BYTES {
        return Err(failure(
            "OAUTH_RESPONSE_LIMIT",
            "The token response exceeded 64 KiB.",
        ));
    }
    let token: TokenResponse = serde_json::from_slice(body).map_err(|_| {
        failure(
            "OAUTH_INVALID_RESPONSE",
            "The token response was not supported OAuth JSON.",
        )
    })?;
    let base = token.access_token.trim_end_matches('=');
    if !token.token_type.eq_ignore_ascii_case("bearer")
        || base.is_empty()
        || !base
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/".contains(&byte))
        || token.access_token.len() > 16_384
        || token.refresh_token.as_ref().is_some_and(|value| {
            value.is_empty() || value.len() > 16_384 || value.chars().any(char::is_control)
        })
    {
        return Err(failure(
            "OAUTH_INVALID_RESPONSE",
            "The provider must return a bounded Bearer token.",
        ));
    }
    Ok(token)
}

async fn callback(
    listener: TcpListener,
    expected: CsrfToken,
    issuer: &str,
) -> Result<String, RunError> {
    loop {
        let (mut stream, _) = listener.accept().await.map_err(|_| {
            failure(
                "OAUTH_CALLBACK_FAILED",
                "The OAuth callback listener failed.",
            )
        })?;
        let parsed = tokio::time::timeout(Duration::from_secs(3), async {
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let count = stream.read(&mut chunk).await.map_err(|_| {
                    failure("OAUTH_CALLBACK_FAILED", "The callback could not be read.")
                })?;
                if count == 0 || bytes.len().saturating_add(count) > CALLBACK_BYTES {
                    return Ok::<_, RunError>(None);
                }
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Ok(parse_callback(&bytes, &expected, issuer))
        })
        .await;
        let Some(code) = parsed.ok().and_then(Result::ok).flatten() else {
            let _ = tokio::time::timeout(
                Duration::from_secs(1),
                stream.write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                ),
            )
            .await;
            continue;
        };
        let _ = tokio::time::timeout(Duration::from_secs(1), stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 49\r\nConnection: close\r\nCache-Control: no-store\r\n\r\nAuthorization received. Return to Velo to finish.")).await;
        return code;
    }
}

fn parse_callback(
    bytes: &[u8],
    expected: &CsrfToken,
    issuer: &str,
) -> Option<Result<String, RunError>> {
    let source = std::str::from_utf8(bytes).ok()?;
    let mut parts = source.lines().next()?.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let path = parts.next()?;
    if !path.starts_with("/oauth/callback?") {
        return None;
    }
    let url = Url::parse(&format!("http://127.0.0.1{path}")).ok()?;
    if url.fragment().is_some() || url.path() != "/oauth/callback" {
        return None;
    }
    let mut state = None;
    let mut code = None;
    let mut error = None;
    let mut response_issuer = None;
    for (name, value) in url.query_pairs() {
        match name.as_ref() {
            "state" if state.is_none() => state = Some(value.into_owned()),
            "code" if code.is_none() => code = Some(value.into_owned()),
            "error" if error.is_none() => error = Some(value.into_owned()),
            "iss" if response_issuer.is_none() => response_issuer = Some(value.into_owned()),
            "state" | "code" | "error" | "iss" => return None,
            _ => {}
        }
    }
    if CsrfToken::new(state?) != *expected {
        return None;
    }
    if response_issuer.as_deref() != Some(issuer) {
        return Some(Err(failure(
            "OAUTH_ISSUER_MISMATCH",
            "The authorization response did not identify the expected issuer. No token exchange was attempted.",
        )));
    }
    if error.is_some() && code.is_some() {
        return None;
    }
    if error.is_some() {
        return Some(Err(failure(
            "OAUTH_AUTHORIZATION_DENIED",
            "The provider denied authorization.",
        )));
    }
    let code = code?;
    if code.is_empty() || code.len() > 4_096 || code.chars().any(char::is_control) {
        return None;
    }
    Some(Ok(code))
}

async fn save<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: OAuthConfig,
    token: TokenResponse,
    cancel: CancellationToken,
) -> Result<OAuthToken, RunError> {
    let result = OAuthToken {
        reference: format!("{{{{secret:oauth.{}}}}}", config.credential_id),
        expires_in: token.expires_in,
        refresh_available: token.refresh_token.is_some(),
    };
    app.state::<History>()
        .configuration(true, move |_| {
            if cancel.is_cancelled() {
                return Err(failure(
                    "OAUTH_CANCELLED",
                    "Token acquisition was cancelled.",
                ));
            }
            let value = serde_json::to_string(&StoredToken {
                access_token: token.access_token,
                refresh_token: token.refresh_token,
                token_endpoint: config.token_endpoint,
                client_id: config.client_id,
                issuer: config.issuer,
            })
            .map_err(|_| {
                failure(
                    "OAUTH_CREDENTIAL_UNAVAILABLE",
                    "The token could not be encoded for secure storage.",
                )
            })?;
            credentials::commit(&entry(&config.credential_id)?, &value, &cancel)?;
            Ok(())
        })
        .await?;
    Ok(result)
}

#[tauri::command]
pub(crate) async fn oauth_begin<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, OAuthState>,
    config: OAuthConfig,
    refresh: bool,
) -> Result<OAuthPending, RunError> {
    validate(&config)?;
    let mut current = state.0.lock().await;
    if current.is_some() {
        return Err(failure(
            "OAUTH_BUSY",
            "Finish or cancel the current OAuth operation.",
        ));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let cancel = CancellationToken::new();
    let owned_cancel = cancel.clone();
    let (send, receive) = oneshot::channel();
    let mut redirect_uri = None;
    let mut authorization = None;
    let mut pkce = None;
    if !refresh && matches!(config.grant, Grant::AuthorizationCode) {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| {
                failure(
                    "OAUTH_CALLBACK_FAILED",
                    "A local OAuth callback port could not be reserved.",
                )
            })?;
        let port = listener
            .local_addr()
            .map_err(|_| {
                failure(
                    "OAUTH_CALLBACK_FAILED",
                    "The local callback address is unavailable.",
                )
            })?
            .port();
        let redirect = format!("http://127.0.0.1:{port}/oauth/callback");
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let csrf = CsrfToken::new_random();
        let url = authorization_request(&config, &redirect, &csrf, &challenge)?;
        authorization = Some(url.to_string());
        redirect_uri = Some(redirect.clone());
        pkce = Some((listener, csrf, verifier, redirect));
    }
    let task = tauri::async_runtime::spawn(async move {
        let runs = app.state::<Arc<DesktopRuns>>();
        let exiting = runs.preview_exit.notified();
        tokio::pin!(exiting);
        exiting.as_mut().enable();
        let result = async {
            let _admission = runs.admission.try_lock().map_err(|_| {
                failure(
                    "REQUEST_BUSY",
                    "Wait for the active request operation before acquiring tokens.",
                )
            })?;
            if runs.closing.load(std::sync::atomic::Ordering::Acquire)
                || runs.manager.current().await.is_some_and(|snapshot| {
                    matches!(
                        snapshot.state,
                        velo_engine::RunState::Starting
                            | velo_engine::RunState::Running
                            | velo_engine::RunState::Stopping
                    )
                })
            {
                return Err(failure(
                    "RUN_ALREADY_ACTIVE",
                    "Stop the load test before acquiring OAuth tokens.",
                ));
            }
            if owned_cancel.is_cancelled() { return Err(failure("OAUTH_CANCELLED", "Token acquisition was cancelled.")); }
            let secret = client_secret(&app, &config).await?;
            if owned_cancel.is_cancelled() { return Err(failure("OAUTH_CANCELLED", "Token acquisition was cancelled.")); }
            let previous_refresh = if refresh { Some(refresh_token(&app, &config).await?) } else { None };
            if runs.closing.load(std::sync::atomic::Ordering::Acquire) { owned_cancel.cancel(); }
            let operation = async {
            let mut parameters = Vec::new();
            if let Some(refresh_token) = &previous_refresh {
                parameters.extend([
                    ("grant_type", "refresh_token".into()),
                    ("refresh_token", refresh_token.clone()),
                ]);
                if !config.scope.is_empty() { parameters.push(("scope", config.scope.clone())); }
            } else if let Some((listener, csrf, verifier, redirect)) = pkce {
                let url = authorization.ok_or_else(|| {
                    failure(
                        "OAUTH_INVALID_CONFIG",
                        "The authorization URL is unavailable.",
                    )
                })?;
                tauri::async_runtime::spawn_blocking(move || {
                    tauri_plugin_opener::open_url(url, None::<&str>)
                })
                .await
                .map_err(|_| {
                    failure(
                        "OAUTH_BROWSER_FAILED",
                        "The authorization browser could not be opened.",
                    )
                })?
                .map_err(|_| {
                    failure(
                        "OAUTH_BROWSER_FAILED",
                        "The authorization browser could not be opened.",
                    )
                })?;
                let code = callback(listener, csrf, config.issuer.as_deref().unwrap_or_default()).await?;
                parameters.extend([
                    ("grant_type", "authorization_code".into()),
                    ("code", code),
                    ("code_verifier", verifier.secret().clone()),
                    ("redirect_uri", redirect),
                ]);
            } else {
                parameters.push(("grant_type", "client_credentials".into()));
                if !config.scope.is_empty() {
                    parameters.push(("scope", config.scope.clone()));
                }
            }
            let mut token = exchange(&config, secret, parameters).await?;
            if token.refresh_token.is_none() {
                token.refresh_token = previous_refresh;
            }
            Ok::<_, RunError>(token)
            };
            let token = tokio::select! {
                biased;
                _ = owned_cancel.cancelled() => Err(failure("OAUTH_CANCELLED", "Token acquisition was cancelled.")),
                _ = &mut exiting => Err(failure("OAUTH_CANCELLED", "Token acquisition was cancelled because Velo is closing.")),
                result = tokio::time::timeout(FLOW_TIMEOUT, operation) => result.unwrap_or_else(|_| Err(failure("OAUTH_TIMEOUT", "OAuth acquisition exceeded three minutes. Start again."))),
            }?;
            let commit = save(&app, config, token, owned_cancel.clone());
            tokio::pin!(commit);
            tokio::select! {
                biased;
                _ = owned_cancel.cancelled() => commit.await,
                _ = &mut exiting => { owned_cancel.cancel(); commit.await },
                result = &mut commit => result,
            }
        }.await;
        let cleanup = match &result {
            Err(error) if error.code == "OAUTH_CREDENTIAL_ROLLBACK_FAILED" => {
                tracing::error!(
                    code = "OAUTH_CREDENTIAL_ROLLBACK_FAILED",
                    "OAuth credential cleanup requires user action"
                );
                Err(error.clone())
            }
            _ => Ok(()),
        };
        let _ = send.send(result);
        owned_cancel.cancel();
        cleanup
    });
    *current = Some(Flow {
        id: id.clone(),
        cancel,
        result: Some(receive),
        task: Some(task),
    });
    Ok(OAuthPending { id, redirect_uri })
}

#[tauri::command]
pub(crate) async fn oauth_complete(
    state: tauri::State<'_, OAuthState>,
    id: String,
) -> Result<OAuthToken, RunError> {
    let receive = {
        let mut current = state.0.lock().await;
        current
            .as_mut()
            .filter(|flow| flow.id == id)
            .and_then(|flow| flow.result.take())
            .ok_or_else(|| failure("OAUTH_FLOW_UNAVAILABLE", "Start a new OAuth operation."))?
    };
    let result = receive
        .await
        .map_err(|_| failure("OAUTH_CANCELLED", "Token acquisition was cancelled."))?;
    let mut current = state.0.lock().await;
    if current.as_ref().is_some_and(|flow| flow.id == id) {
        *current = None;
    }
    result
}

#[tauri::command]
pub(crate) async fn oauth_cancel(
    state: tauri::State<'_, OAuthState>,
    id: String,
) -> Result<(), RunError> {
    let flow = {
        let mut current = state.0.lock().await;
        if current.as_ref().is_some_and(|flow| flow.id == id) {
            current.take()
        } else {
            None
        }
    };
    if let Some(mut flow) = flow {
        flow.cancel.cancel();
        if let Some(task) = flow.task.take() {
            task.await.map_err(|_| {
                failure("OAUTH_CLEANUP_FAILED", "OAuth cleanup did not complete.")
            })??;
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn oauth_forget(
    history: tauri::State<'_, History>,
    state: tauri::State<'_, OAuthState>,
    credential_id: String,
) -> Result<(), RunError> {
    if credential_id.is_empty()
        || credential_id.len() > 40
        || !credential_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(failure(
            "OAUTH_INVALID_CONFIG",
            "Use a valid OAuth credential ID.",
        ));
    }
    let current = state.0.lock().await;
    if current
        .as_ref()
        .is_some_and(|flow| !flow.cancel.is_cancelled())
    {
        return Err(failure(
            "OAUTH_BUSY",
            "Finish or cancel token acquisition before forgetting credentials.",
        ));
    }
    history
        .configuration(true, move |_| {
            match entry(&credential_id)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err(failure(
                    "OAUTH_CREDENTIAL_UNAVAILABLE",
                    "The local OAuth token could not be deleted.",
                )),
            }
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_requires_exact_state_path_and_unique_fields() {
        let expected = CsrfToken::new("secret-state".into());
        for path in [
            "/oauth/callback?code=secret&state=wrong",
            "/other?code=secret&state=secret-state",
            "/oauth/callback?code=a&code=b&state=secret-state",
            "/oauth/callback?code=secret&state=secret-state&state=secret-state",
        ] {
            assert!(
                parse_callback(
                    format!("GET {path} HTTP/1.1\r\n\r\n").as_bytes(),
                    &expected,
                    "https://issuer.example"
                )
                .is_none()
            );
        }
        assert_eq!(
            parse_callback(
                b"GET /oauth/callback?code=secret&state=secret-state&iss=https%3A%2F%2Fissuer.example HTTP/1.1\r\n\r\n",
                &expected, "https://issuer.example"
            )
            .unwrap()
            .unwrap(),
            "secret"
        );
        assert_eq!(
            parse_callback(
                b"GET /oauth/callback?error=access_denied&state=secret-state&iss=https%3A%2F%2Fissuer.example HTTP/1.1\r\n\r\n",
                &expected, "https://issuer.example"
            )
            .unwrap()
            .unwrap_err()
            .code,
            "OAUTH_AUTHORIZATION_DENIED"
        );
    }

    #[tokio::test]
    async fn loopback_callback_rejects_spoofing_and_releases_its_listener() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(callback(
            listener,
            CsrfToken::new("expected-state".into()),
            "https://issuer.example",
        ));
        for (state, status) in [("wrong-state", "400"), ("expected-state", "200")] {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream.write_all(format!("GET /oauth/callback?code=authorization-code&state={state}&iss=https%3A%2F%2Fissuer.example HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes()).await.unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).await.unwrap();
            assert!(response.starts_with(&format!("HTTP/1.1 {status}")));
            assert!(!response.contains("authorization-code"));
        }
        assert_eq!(task.await.unwrap().unwrap(), "authorization-code");
        assert!(tokio::net::TcpStream::connect(address).await.is_err());

        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(callback(
            listener,
            CsrfToken::new("state".into()),
            "https://issuer.example",
        ));
        task.abort();
        let _ = task.await;
        assert!(tokio::net::TcpStream::connect(address).await.is_err());
    }

    #[test]
    fn oauth_endpoints_cannot_redirect_or_expose_credentials() {
        for source in [
            "http://example.com/token",
            "https://user:secret@example.com/token",
            "https://example.com/token?secret=value",
            "https://example.com/token#fragment",
            "file:///token",
        ] {
            let error = endpoint(source).unwrap_err();
            assert_eq!(error.code, "OAUTH_INVALID_CONFIG");
            assert!(!error.message.contains("secret=value"));
        }
        assert!(endpoint("https://example.com/token").is_ok());
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;

    fn config() -> OAuthConfig {
        OAuthConfig {
            credential_id: "test".into(),
            grant: Grant::AuthorizationCode,
            token_endpoint: "https://issuer.example/token?tenant=development".into(),
            authorization_endpoint: Some(
                "https://issuer.example/authorize?tenant=development".into(),
            ),
            issuer: Some("https://issuer.example".into()),
            client_id: "client".into(),
            client_secret_name: None,
            scope: "read write".into(),
        }
    }

    #[test]
    fn authorization_request_preserves_tenant_and_binds_pkce_without_empty_scope() {
        let mut config = config();
        config.scope.clear();
        let csrf = CsrfToken::new("state".into());
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let request = authorization_request(
            &config,
            "http://127.0.0.1:1234/oauth/callback",
            &csrf,
            &challenge,
        )
        .unwrap();
        let parameters: std::collections::BTreeMap<_, _> =
            request.query_pairs().into_owned().collect();
        assert_eq!(
            parameters.get("tenant").map(String::as_str),
            Some("development")
        );
        assert_eq!(
            parameters.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            parameters.get("code_challenge").map(String::as_str),
            Some(challenge.as_str())
        );
        assert_eq!(
            parameters.get("redirect_uri").map(String::as_str),
            Some("http://127.0.0.1:1234/oauth/callback")
        );
        assert!(!parameters.contains_key("scope"));
        assert!(!request.as_str().contains(verifier.secret()));
        config.scope = "read write".into();
        let request = authorization_request(
            &config,
            "http://127.0.0.1:1234/oauth/callback",
            &csrf,
            &challenge,
        )
        .unwrap();
        assert!(
            request
                .query_pairs()
                .any(|(name, value)| name == "scope" && value == "read write")
        );
    }

    #[test]
    fn callback_rejects_missing_mismatched_and_duplicate_issuers_before_exchange() {
        let expected = CsrfToken::new("state".into());
        for parameters in ["", "&iss=https%3A%2F%2Fother.example"] {
            let request =
                format!("GET /oauth/callback?code=code&state=state{parameters} HTTP/1.1\r\n\r\n");
            let error = parse_callback(request.as_bytes(), &expected, "https://issuer.example")
                .unwrap()
                .unwrap_err();
            assert_eq!(error.code, "OAUTH_ISSUER_MISMATCH");
        }
        for parameters in [
            "&iss=https%3A%2F%2Fissuer.example&iss=https%3A%2F%2Fissuer.example",
            "&iss=https%3A%2F%2Fissuer.example&error=access_denied",
            "&iss=https%3A%2F%2Fissuer.example#fragment",
        ] {
            let request =
                format!("GET /oauth/callback?code=code&state=state{parameters} HTTP/1.1\r\n\r\n");
            assert!(
                parse_callback(request.as_bytes(), &expected, "https://issuer.example").is_none()
            );
        }
    }

    #[test]
    fn config_preserves_static_queries_and_rejects_oauth_parameter_override_or_invalid_scope() {
        let mut config = config();
        assert!(validate(&config).is_ok());
        for query in [
            "state=attacker",
            "client_secret=private",
            "%63ode=stolen",
            "scope=admin",
        ] {
            config.token_endpoint = format!("https://issuer.example/token?{query}");
            assert_eq!(validate(&config).unwrap_err().code, "OAUTH_INVALID_CONFIG");
        }
        config.token_endpoint = "https://issuer.example/token".into();
        for scope in [
            " read",
            "read  write",
            "read\"",
            "read\\",
            "read\nwrite",
            "résumé",
        ] {
            config.scope = scope.into();
            assert_eq!(validate(&config).unwrap_err().code, "OAUTH_INVALID_CONFIG");
        }
        config.scope.clear();
        config.issuer = None;
        assert!(validate(&config).is_err());
        config.grant = Grant::ClientCredentials;
        config.client_secret_name = Some("client-secret".into());
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn token_request_uses_form_encoded_basic_credentials_and_preserves_endpoint_query() {
        let mut config = config();
        config.client_id = "client:id with space".into();
        let request = token_request(
            &reqwest::Client::new(),
            &config,
            Some("secret:with space".into()),
            vec![("grant_type", "client_credentials".into())],
        )
        .unwrap()
        .build()
        .unwrap();
        assert_eq!(request.url().query(), Some("tenant=development"));
        assert_eq!(
            request
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .unwrap(),
            "Basic Y2xpZW50JTNBaWQrd2l0aCtzcGFjZTpzZWNyZXQlM0F3aXRoK3NwYWNl"
        );
        assert_eq!(
            request.body().and_then(reqwest::Body::as_bytes).unwrap(),
            b"grant_type=client_credentials"
        );
        let request = token_request(
            &reqwest::Client::new(),
            &config,
            None,
            vec![
                ("grant_type", "authorization_code".into()),
                ("code_verifier", "private-verifier".into()),
            ],
        )
        .unwrap()
        .build()
        .unwrap();
        assert!(
            !request
                .headers()
                .contains_key(reqwest::header::AUTHORIZATION)
        );
        let body =
            std::str::from_utf8(request.body().and_then(reqwest::Body::as_bytes).unwrap()).unwrap();
        assert!(body.contains("client_id=client%3Aid+with+space"));
        assert!(body.contains("code_verifier=private-verifier"));
    }

    #[test]
    fn token_response_rejects_unsupported_or_malformed_credentials_without_echoing_them() {
        assert!(
            parse_token_response(
                br#"{"access_token":"safe-token","token_type":"Bearer","extension":"ignored"}"#
            )
            .is_ok()
        );
        for source in [
            r#"{"access_token":"private token","token_type":"Bearer"}"#,
            r#"{"access_token":"private","token_type":"DPoP"}"#,
            r#"{"access_token":"private","token_type":"Bearer","refresh_token":"\nprivate"}"#,
            r#"{"access_token":"private","token_type":"Bearer","expires_in":-1}"#,
        ] {
            let error = parse_token_response(source.as_bytes()).err().unwrap();
            assert_eq!(error.code, "OAUTH_INVALID_RESPONSE");
            assert!(!error.to_string().contains("private"));
        }
        assert_eq!(
            parse_token_response(&vec![b'x'; TOKEN_BYTES + 1])
                .err()
                .unwrap()
                .code,
            "OAUTH_RESPONSE_LIMIT"
        );
    }

    #[test]
    fn refresh_credentials_bind_to_endpoint_client_and_issuer() {
        let config = config();
        let mut stored = StoredToken {
            access_token: "access".into(),
            refresh_token: Some("refresh".into()),
            token_endpoint: config.token_endpoint.clone(),
            client_id: config.client_id.clone(),
            issuer: config.issuer.clone(),
        };
        assert!(validate_refresh(&stored, &config).is_ok());
        stored.issuer = Some("https://other.example".into());
        assert_eq!(
            validate_refresh(&stored, &config).unwrap_err().code,
            "OAUTH_CONFIG_CHANGED"
        );
        stored.issuer = config.issuer.clone();
        stored.token_endpoint = "https://other.example/token".into();
        assert!(validate_refresh(&stored, &config).is_err());
        stored.token_endpoint = config.token_endpoint.clone();
        stored.client_id = "other-client".into();
        assert!(validate_refresh(&stored, &config).is_err());
    }
}

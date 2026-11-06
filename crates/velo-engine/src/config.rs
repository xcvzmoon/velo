use std::time::Duration;

use reqwest::{Client, Method, Url, header};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::workload::{Schedule, Workload};

/// Named request value. Duplicate query parameters and headers retain their order.
/// Debug formatting omits both the name and value.
///
/// # Examples
/// ```
/// use velo_engine::NameValue;
/// let header = NameValue { name: "accept".into(), value: "application/json".into() };
/// assert_eq!(header.name, "accept");
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct NameValue {
    /// Header or query parameter name.
    pub name: String,
    /// Header or query parameter value; never included in run reports.
    pub value: String,
}

impl std::fmt::Debug for NameValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NameValue([redacted])")
    }
}

/// Supported logical request methods.
///
/// # Examples
/// ```
/// let method: velo_engine::RequestMethod = serde_json::from_str("\"POST\"")?;
/// assert!(matches!(method, velo_engine::RequestMethod::POST));
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, TS)]
pub enum RequestMethod {
    /// Fetch a response without a request body.
    #[default]
    GET,
    /// Submit an optional text or JSON body.
    POST,
    /// Replace a resource with an optional body.
    PUT,
    /// Modify a resource with an optional body.
    PATCH,
    /// Delete a resource with an optional body.
    DELETE,
    /// Fetch headers without a request or response body.
    HEAD,
    /// Query supported communication options, with an optional body.
    OPTIONS,
}

/// Request payload. JSON is validated before execution and sent unchanged.
/// Debug formatting omits payload content.
///
/// # Examples
/// ```
/// let body = velo_engine::RequestBody::Json(r#"{"enabled":true}"#.into());
/// let encoded = serde_json::to_value(body)?;
/// assert_eq!(encoded["kind"], "json");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Default, Deserialize, Serialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RequestBody {
    /// No payload.
    #[default]
    None,
    /// UTF-8 text, with a default text/plain content type.
    Text(String),
    /// UTF-8 JSON, with a default application/json content type.
    Json(String),
    /// URL-encoded pairs; duplicate names retain their order.
    Form(#[ts(type = "NameValue[]")] Vec<NameValue>),
    /// Ordered text and streamed file parts. Files must remain unchanged during a run.
    Multipart(#[ts(type = "MultipartPart[]")] Vec<crate::MultipartPart>),
}

impl std::fmt::Debug for RequestBody {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::None => "RequestBody::None",
            Self::Text(_) => "RequestBody::Text([redacted])",
            Self::Json(_) => "RequestBody::Json([redacted])",
            Self::Form(_) => "RequestBody::Form([redacted])",
            Self::Multipart(_) => "RequestBody::Multipart([redacted])",
        })
    }
}

/// Explicit local allocation budgets, adjustable before starting a run.
///
/// Defaults allow 256 workers and an 8-MiB request-data budget across the
/// template, VU request clones, stage schedule, and threshold rules. Shared bodies
/// are conservatively counted per VU. These are allocation safeguards, not a measured
/// capacity claim. Streaming uploads reserve two 16-KiB buffers per file per VU.
/// HTTP transport and runtime overhead also consume memory outside these budgets.
///
/// # Examples
/// ```
/// let budget = velo_engine::ResourceBudget { max_vus: 500, ..Default::default() };
/// assert_eq!(budget.max_request_bytes, 8 * 1024 * 1024);
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudget {
    /// Maximum number of simultaneously owned VU tasks; must be positive.
    pub max_vus: u32,
    /// Maximum normalized request bytes across the template and VU clones,
    /// including pair overhead, the stage schedule, threshold rules, and upload buffers.
    /// This is not a process-wide memory limit.
    pub max_request_bytes: u32,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            max_vus: 256,
            max_request_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Untrusted request and workload input, validated once by [`crate::RunManager::prepare`]
/// or [`crate::RunManager::start`]. Durations are integer milliseconds.
/// Debug formatting exposes workload settings but omits request data.
///
/// # Examples
/// ```
/// use velo_engine::RunConfig;
/// let config = RunConfig { url: "http://127.0.0.1:8080/health".into(), ..RunConfig::default() };
/// assert_eq!(config.timeout_ms, 30_000);
/// ```
#[derive(Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RunConfig {
    /// Absolute HTTP(S) URL without embedded credentials or a fragment.
    pub url: String,
    /// Logical HTTP method. GET and HEAD reject request bodies.
    pub method: RequestMethod,
    /// Custom headers; Host, Content-Length, and Transfer-Encoding are reserved.
    #[ts(type = "NameValue[]")]
    pub headers: Vec<NameValue>,
    /// Additional query pairs appended to the URL, including duplicate names.
    #[ts(type = "NameValue[]")]
    pub query: Vec<NameValue>,
    /// Optional body for methods other than GET and HEAD.
    pub body: RequestBody,
    /// Explicit authentication; custom Authorization headers cannot be combined with it.
    pub auth: crate::RequestAuth,
    /// Negotiated HTTP by default; HTTP/2 forces h2, including prior knowledge for cleartext.
    pub protocol: crate::HttpProtocol,
    /// Maximum redirects to follow; zero disables redirects. Cross-origin redirects and
    /// HTTPS downgrades are rejected, keeping custom credentials and payloads on the target origin.
    pub max_redirects: u32,
    /// Constant, staged, or arrival schedule, validated before worker allocation.
    pub workload: Workload,
    /// Full response timeout, including streamed body; 1–3,600,000 ms.
    pub timeout_ms: u32,
    /// Delay after an iteration finishes, before another starts; zero by default.
    pub pacing_ms: u32,
    /// Maximum drain time after stop or duration expiry; zero aborts immediately.
    pub grace_ms: u32,
    /// Final cumulative metric rules; empty by default.
    #[ts(type = "Threshold[]")]
    pub thresholds: Vec<crate::Threshold>,
    /// Explicit task and retained request-data limits.
    pub budget: ResourceBudget,
}

impl std::fmt::Debug for RunConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RunConfig")
            .field("method", &self.method)
            .field("protocol", &self.protocol)
            .field("max_redirects", &self.max_redirects)
            .field("workload", &self.workload)
            .field("timeout_ms", &self.timeout_ms)
            .field("pacing_ms", &self.pacing_ms)
            .field("grace_ms", &self.grace_ms)
            .field("budget", &self.budget)
            .finish_non_exhaustive()
    }
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            method: RequestMethod::GET,
            headers: Vec::new(),
            query: Vec::new(),
            body: RequestBody::None,
            auth: crate::RequestAuth::None,
            protocol: crate::HttpProtocol::Auto,
            max_redirects: 0,
            workload: Workload::default(),
            timeout_ms: 30_000,
            pacing_ms: 0,
            grace_ms: 5_000,
            thresholds: Vec::new(),
            budget: ResourceBudget::default(),
        }
    }
}

/// Safe, stable application failure. Input values and HTTP error details are omitted.
///
/// # Examples
/// ```
/// let error = velo_engine::RunManager::prepare(Default::default()).unwrap_err();
/// assert_eq!(error.code, "INVALID_RUN_CONFIG");
/// assert_eq!(error.field.as_deref(), Some("url"));
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS, thiserror::Error)]
#[error("{code}: {message}")]
pub struct RunError {
    /// Stable machine-readable code.
    pub code: String,
    /// Safe explanation suitable for display.
    pub message: String,
    /// Invalid configuration field, when applicable.
    pub field: Option<String>,
}

impl RunError {
    pub(crate) fn new(code: &str, message: &str, field: Option<&str>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            field: field.map(str::to_owned),
        }
    }
}

/// Validated confirmation details; preparing a run sends no HTTP traffic.
///
/// # Examples
/// ```
/// let config = velo_engine::RunConfig {
///     url: "https://example.com/private?token=secret".into(),
///     ..Default::default()
/// };
/// let confirmation = velo_engine::RunManager::prepare(config)?;
/// assert_eq!(confirmation.origin, "https://example.com");
/// # Ok::<(), velo_engine::RunError>(())
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct RunConfirmation {
    /// Target scheme, host, and port; excludes path, credentials, and query.
    pub origin: String,
    /// Peak requested workers, including the starting count.
    pub vus: u32,
    /// Requested scheduling duration in milliseconds.
    pub duration_ms: u32,
    /// Native-validated schedule shown during authorization review.
    pub workload: Workload,
    /// Explicit allocation budgets.
    pub budget: ResourceBudget,
    /// HIGH_VU_COUNT warns at 100 workers; HIGH_ARRIVAL_RATE at 1,000 iterations/s.
    #[ts(type = "string[]")]
    pub warnings: Vec<String>,
}

pub(crate) struct ValidatedConfig {
    pub confirmation: RunConfirmation,
    pub thresholds: Vec<crate::Threshold>,
    pub request: reqwest::Request,
    pub multipart: Option<Vec<crate::http_options::PreparedPart>>,
    pub max_redirects: u32,
    pub timeout: Duration,
    pub uploads: tokio::sync::Mutex<tokio::task::JoinSet<()>>,
    pub upload_stop: tokio_util::sync::CancellationToken,
    pub client: Client,
    pub schedule: Schedule,
    pub duration: Duration,
    pub pacing: Duration,
    pub grace: Duration,
}

pub(crate) fn validate(config: RunConfig) -> Result<ValidatedConfig, RunError> {
    let invalid = |message, field| RunError::new("INVALID_RUN_CONFIG", message, Some(field));
    crate::threshold::validate(&config.thresholds)?;
    let threshold_bytes = config
        .thresholds
        .len()
        .saturating_mul(std::mem::size_of::<crate::Threshold>());
    let schedule = Schedule::new(
        &config.workload,
        config.budget.max_vus,
        config.budget.max_request_bytes,
    )?;
    if matches!(config.workload, Workload::Arrival { .. }) && config.pacing_ms != 0 {
        return Err(invalid(
            "Arrival workloads require zero iteration pacing.",
            "pacing_ms",
        ));
    }
    if !(1..=3_600_000).contains(&config.timeout_ms) {
        return Err(invalid(
            "Timeout must be between 1 ms and one hour.",
            "timeout_ms",
        ));
    }

    let mut size = config
        .url
        .len()
        .saturating_add(schedule.retained_bytes())
        .saturating_add(threshold_bytes);
    for pair in config.headers.iter().chain(&config.query) {
        size = size
            .saturating_add(std::mem::size_of::<NameValue>())
            .saturating_add(pair.name.len())
            .saturating_add(pair.value.len());
    }
    let body_size = crate::http_options::body_size(&config.body);
    size = size.saturating_add(crate::http_options::auth_size(&config.auth));
    size = size.saturating_add(body_size);
    if size > config.budget.max_request_bytes as usize || config.budget.max_request_bytes == 0 {
        return Err(RunError::new(
            "REQUEST_BUDGET_EXCEEDED",
            "Request configuration exceeds its byte budget.",
            Some("budget.max_request_bytes"),
        ));
    }

    let mut url =
        Url::parse(&config.url).map_err(|_| invalid("Enter an absolute HTTP(S) URL.", "url"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(invalid("Enter an absolute HTTP(S) URL.", "url"));
    }
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(invalid(
            "URL credentials and fragments are not supported.",
            "url",
        ));
    }
    let origin = url.origin().ascii_serialization();
    if !config.query.is_empty() {
        let mut query = url.query_pairs_mut();
        for NameValue { name, value } in &config.query {
            query.append_pair(name, value);
        }
    }
    let mut headers = header::HeaderMap::new();
    for NameValue { name, value } in &config.headers {
        let name = header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| invalid("Invalid HTTP header name.", "headers"))?;
        if matches!(
            name,
            header::HOST | header::CONTENT_LENGTH | header::TRANSFER_ENCODING
        ) {
            return Err(invalid(
                "Host and body framing headers are managed by the client.",
                "headers",
            ));
        }
        let value = header::HeaderValue::from_str(value)
            .map_err(|_| invalid("Invalid HTTP header value.", "headers"))?;
        headers.try_append(name, value).map_err(|_| {
            invalid(
                "Too many HTTP headers for the transport representation.",
                "headers",
            )
        })?;
    }
    let method = match config.method {
        RequestMethod::GET => Method::GET,
        RequestMethod::POST => Method::POST,
        RequestMethod::PUT => Method::PUT,
        RequestMethod::PATCH => Method::PATCH,
        RequestMethod::DELETE => Method::DELETE,
        RequestMethod::HEAD => Method::HEAD,
        RequestMethod::OPTIONS => Method::OPTIONS,
    };
    if matches!(method, Method::GET | Method::HEAD) && !matches!(config.body, RequestBody::None) {
        return Err(invalid(
            "GET and HEAD requests cannot contain a body.",
            "body",
        ));
    }
    if !matches!(config.auth, crate::RequestAuth::None)
        && headers.contains_key(header::AUTHORIZATION)
    {
        return Err(invalid(
            "Choose authentication or a custom Authorization header, not both.",
            "auth",
        ));
    }
    if matches!(config.body, RequestBody::Multipart(_))
        && headers.contains_key(header::CONTENT_TYPE)
    {
        return Err(invalid(
            "Multipart Content-Type and boundary are managed by the client.",
            "headers",
        ));
    }
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .pool_max_idle_per_host(schedule.peak_vus as usize)
        .timeout(Duration::from_millis(config.timeout_ms.into()));
    let client = match config.protocol {
        crate::HttpProtocol::Auto => client,
        crate::HttpProtocol::Http1 => client.http1_only(),
        crate::HttpProtocol::Http2 => client.http2_prior_knowledge(),
    }
    .build()
    .map_err(|_| {
        RunError::new(
            "HTTP_CLIENT_INIT_FAILED",
            "HTTP client initialization failed.",
            None,
        )
    })?;
    if !headers.contains_key(header::CONTENT_TYPE) {
        let content_type = match &config.body {
            RequestBody::Text(_) => Some("text/plain; charset=utf-8"),
            RequestBody::Json(_) => Some("application/json"),
            RequestBody::Form(_) => Some("application/x-www-form-urlencoded"),
            RequestBody::None | RequestBody::Multipart(_) => None,
        };
        if let Some(content_type) = content_type {
            headers
                .try_insert(
                    header::CONTENT_TYPE,
                    header::HeaderValue::from_static(content_type),
                )
                .map_err(|_| {
                    invalid(
                        "Too many HTTP headers for the transport representation.",
                        "headers",
                    )
                })?;
        }
    }
    let mut request = client.request(method, url).headers(headers);
    request = crate::http_options::authenticate(request, &config.auth)?;
    let mut multipart = None;
    match config.body {
        RequestBody::None => {}
        RequestBody::Form(pairs) => {
            let pairs: Vec<_> = pairs
                .into_iter()
                .map(|pair| (pair.name, pair.value))
                .collect();
            request = request.form(&pairs);
        }
        RequestBody::Multipart(parts) => {
            multipart = Some(crate::http_options::prepare_parts(parts)?);
        }
        RequestBody::Text(value) => {
            request = request.body(value);
        }
        RequestBody::Json(value) => {
            serde_json::from_str::<serde_json::Value>(&value)
                .map_err(|_| invalid("Enter a valid JSON body.", "body"))?;
            request = request.body(value);
        }
    }
    let request = request
        .build()
        .map_err(|_| invalid("Invalid HTTP request settings.", "headers"))?;
    let normalized_body_bytes = request
        .body()
        .and_then(reqwest::Body::as_bytes)
        .map_or(0, <[u8]>::len);
    let mut request_bytes = request
        .url()
        .as_str()
        .len()
        .saturating_add(body_size.max(normalized_body_bytes));
    for (name, value) in request.headers() {
        request_bytes = request_bytes
            .saturating_add(std::mem::size_of::<NameValue>())
            .saturating_add(name.as_str().len())
            .saturating_add(value.len());
    }
    let upload_bytes = crate::http_options::upload_buffer_bytes(multipart.as_deref());
    let retained_bytes = request_bytes
        .saturating_mul(schedule.peak_vus as usize + 1)
        .saturating_add(schedule.retained_bytes())
        .saturating_add(threshold_bytes)
        .saturating_add(upload_bytes.saturating_mul(schedule.peak_vus as usize));
    if retained_bytes > config.budget.max_request_bytes as usize {
        return Err(RunError::new(
            "REQUEST_BUDGET_EXCEEDED",
            "Normalized request data and VU clones exceed the byte budget.",
            Some("budget.max_request_bytes"),
        ));
    }
    Ok(ValidatedConfig {
        confirmation: RunConfirmation {
            origin,
            vus: schedule.peak_vus,
            duration_ms: schedule.duration_ms,
            workload: config.workload.clone(),
            budget: config.budget,
            warnings: {
                let mut warnings = Vec::new();
                if schedule.peak_vus >= 100 {
                    warnings.push("HIGH_VU_COUNT".into());
                }
                if matches!(config.workload, Workload::Arrival { rate: 1000.., .. }) {
                    warnings.push("HIGH_ARRIVAL_RATE".into());
                }
                warnings
            },
        },
        thresholds: config.thresholds,
        multipart,
        max_redirects: config.max_redirects,
        timeout: Duration::from_millis(config.timeout_ms.into()),
        uploads: tokio::sync::Mutex::new(tokio::task::JoinSet::new()),
        upload_stop: tokio_util::sync::CancellationToken::new(),
        request,
        client,
        duration: Duration::from_millis(schedule.duration_ms.into()),
        schedule,
        pacing: Duration::from_millis(config.pacing_ms.into()),
        grace: Duration::from_millis(config.grace_ms.into()),
    })
}

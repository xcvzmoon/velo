//! Sequential functional HTTP inspection with isolated, bounded session state.

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::{Extraction, NameValue, PreviewResponse, RunConfig, RunError, ScenarioStep};

/// Cooperative cancellation that waits for owned upload producers before returning.
///
/// # Examples
/// ```
/// let cancel = velo_engine::ApiCancellation::default();
/// cancel.cancel();
/// assert!(cancel.is_cancelled());
/// ```
#[derive(Clone, Debug, Default)]
pub struct ApiCancellation(CancellationToken);

impl ApiCancellation {
    /// Cancels current and subsequent work sharing this handle.
    ///
    /// # Returns
    /// No value. Cancellation is idempotent and never sends traffic.
    ///
    /// # Examples
    /// ```
    /// velo_engine::ApiCancellation::default().cancel();
    /// ```
    pub fn cancel(&self) {
        self.0.cancel();
    }

    /// Reports whether cancellation has been requested.
    ///
    /// # Returns
    /// True after any clone has been cancelled.
    ///
    /// # Examples
    /// ```
    /// assert!(!velo_engine::ApiCancellation::default().is_cancelled());
    /// ```
    pub fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }

    /// Waits without polling until cancellation is requested.
    ///
    /// # Returns
    /// No value; dropping the wait releases its registration.
    ///
    /// # Examples
    /// ```
    /// # async fn example() {
    /// let cancel = velo_engine::ApiCancellation::default();
    /// cancel.cancel();
    /// cancel.cancelled().await;
    /// # }
    /// ```
    pub async fn cancelled(&self) {
        self.0.cancelled().await;
    }
}

/// Interactive session state, independent of virtual users and other environments.
///
/// # Examples
/// ```
/// assert!(velo_engine::ApiSession::default().view().variables.is_empty());
/// ```
#[derive(Clone, Default)]
pub struct ApiSession(crate::scenario_runtime::Session);

impl std::fmt::Debug for ApiSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ApiSession([redacted])")
    }
}

/// Safe session inspection; values are never exposed or persisted.
///
/// # Examples
/// ```
/// let view = velo_engine::ApiSession::default().view();
/// assert!(view.cookies.is_empty());
/// ```
#[derive(Clone, Debug, Default, Deserialize, Serialize, TS)]
pub struct ApiSessionView {
    /// Extracted variable names; values remain in memory only.
    #[ts(type = "string[]")]
    pub variables: Vec<String>,
    /// Cookie metadata, with masked values and domain/path scope.
    #[ts(type = "NameValue[]")]
    pub cookies: Vec<NameValue>,
}

impl ApiSession {
    /// Inspects names and masked cookie metadata without revealing session values.
    ///
    /// # Returns
    /// A bounded safe view. No credentials or extracted values are copied out.
    ///
    /// # Examples
    /// ```
    /// let session = velo_engine::ApiSession::default();
    /// assert!(session.view().variables.is_empty());
    /// ```
    pub fn view(&self) -> ApiSessionView {
        ApiSessionView {
            variables: self.0.values.keys().cloned().collect(),
            cookies: self.0.cookie_metadata(),
        }
    }
}

/// Functional execution state; skipped steps are retained to reconcile totals.
///
/// # Examples
/// ```
/// let _state = velo_engine::FunctionalState::Passed;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FunctionalState {
    /// A successful status and all configured checks and extractions passed.
    Passed,
    /// Transport, status, assertion, or extraction failed.
    Failed,
    /// Owned work was cancelled before completion.
    Cancelled,
    /// Failure policy or cancellation prevented this step from starting.
    Skipped,
}

/// One functional outcome; response evidence is bounded and redacted before leaving the engine.
///
/// # Examples
/// ```
/// # fn example(result: velo_engine::FunctionalResult) {
/// assert!(result.iteration >= 1);
/// # }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
pub struct FunctionalResult {
    /// Saved request identity.
    pub request_id: String,
    /// Authored display name, never a resolved target URL.
    pub name: String,
    /// One-based iteration number.
    pub iteration: u32,
    /// Final step state.
    pub state: FunctionalState,
    /// Bounded masked response, absent before headers or for cancellation.
    pub response: Option<PreviewResponse>,
    /// Stable safe failure code, without response values.
    pub error: Option<String>,
}

/// Discriminator that keeps functional evidence separate from performance reports.
///
/// # Examples
/// ```
/// assert_eq!(serde_json::to_value(velo_engine::FunctionalKind::Functional)?, "functional");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FunctionalKind {
    /// Sequential API execution, without throughput metrics or performance baselines.
    Functional,
}

/// Lifecycle of an authorized sequential functional operation.
///
/// # Examples
/// ```
/// let _state = velo_engine::FunctionalRunState::Completed;
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FunctionalRunState {
    /// Preflight completed and step progress is being reported.
    Running,
    /// Execution finished; inspect step outcomes for failures and skips.
    Completed,
    /// Cancellation stopped execution and prevented subsequent steps.
    Cancelled,
}

/// Functional report kept separate from load metrics and baselines.
///
/// # Examples
/// ```
/// # fn example(run: velo_engine::FunctionalRun) {
/// assert!(matches!(run.kind, velo_engine::FunctionalKind::Functional));
/// # }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
pub struct FunctionalRun {
    /// Always `functional`, including exported evidence.
    pub kind: FunctionalKind,
    /// Operation identity, also used for cancellation.
    pub id: String,
    /// Collection identity used to reopen a saved request without sending it.
    pub collection_id: String,
    /// Selected environment, or shared values when absent.
    pub environment: Option<String>,
    /// `running`, `completed`, or `cancelled`.
    pub state: FunctionalRunState,
    /// Planned step count, including skipped outcomes.
    pub total: u32,
    /// Portable authored snapshots for history restoration, without resolved secrets or local file paths.
    #[ts(type = "CollectionRequest[]")]
    pub requests: Vec<crate::CollectionRequest>,
    /// Outcomes in displayed execution order.
    #[ts(type = "FunctionalResult[]")]
    pub results: Vec<FunctionalResult>,
    /// Safe final session view; never a persisted cookie jar.
    pub session: ApiSessionView,
    /// Local persistence failure, if execution completed but history could not be retained.
    pub storage_error: Option<String>,
}

/// Executes one request through shared transport, checks, extraction, and cookie rules.
///
/// # Arguments
/// * `config` - Environment-resolved HTTP definition. Session references remain unbound.
/// * `extractions` - Shared bounded selectors, applied atomically to the session.
/// * `secrets` - Resolved secret values to mask in evidence.
/// * `session` - Isolated in-memory state owned by the caller.
/// * `cancel` - Cancellation handle; cleanup completes before returning.
///
/// # Returns
/// Redacted response and an optional extraction failure code. HTTP/check failures remain inspectable.
/// No retries occur. Workload settings do not affect functional execution.
///
/// # Errors
/// Returns shared validation errors before traffic, `INSECURE_AUTH_TARGET` for cleartext
/// remote credentials (including session cookies), `API_CANCELLED`, `API_TIMEOUT`,
/// `API_TRANSPORT_<CATEGORY>`, or a shared session binding/budget failure code.
/// Transport categories report measured failures such as `CONNECT` or `CONNECTION_REFUSED`;
/// request values and infrastructure error text are omitted.
/// Cancelling an operation invalidates its extraction targets but retains received cookies.
/// The caller must enforce target authorization and shared resource admission.
///
/// # Examples
/// ```no_run
/// # async fn example() -> Result<(), velo_engine::RunError> {
/// let mut session = velo_engine::ApiSession::default();
/// let cancel = velo_engine::ApiCancellation::default();
/// let config = velo_engine::RunConfig { url: "https://example.com".into(), ..Default::default() };
/// let (response, _) = velo_engine::send_api_request(config, &[], &[], &mut session, &cancel).await?;
/// assert!(response.status >= 100);
/// # Ok(()) }
/// ```
pub async fn send_api_request(
    config: RunConfig,
    extractions: &[Extraction],
    secrets: &[String],
    session: &mut ApiSession,
    cancel: &ApiCancellation,
) -> Result<(PreviewResponse, Option<String>), RunError> {
    let result = send(config, extractions, secrets, session, cancel).await;
    if result.is_err() {
        session.0.invalidate(extractions);
    }
    result
}

async fn send(
    mut config: RunConfig,
    extractions: &[Extraction],
    secrets: &[String],
    session: &mut ApiSession,
    cancel: &ApiCancellation,
) -> Result<(PreviewResponse, Option<String>), RunError> {
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    if config.scenarios.is_some() || config.data.is_some() {
        return Err(RunError::new(
            "API_REQUEST_REQUIRED",
            "Select a single HTTP request.",
            None,
        ));
    }
    config.workload = crate::Workload::Constant {
        vus: 1,
        duration_ms: 1_000,
    };
    config.thresholds.clear();
    config.abort_policies.clear();
    config.warmup_ms = 0;
    config.pacing_ms = 0;
    let step = ScenarioStep {
        id: "request".into(),
        request: crate::ScenarioRequest {
            url: config.url.clone(),
            method: config.method,
            headers: config.headers.clone(),
            query: config.query.clone(),
            body: config.body.clone(),
            auth: config.auth.clone(),
            assertions: config.assertions.clone(),
        },
        extractions: extractions.to_vec(),
        think_ms: 0,
        on_failure: crate::StepFailure::Stop,
    };
    let mut values = secrets.to_vec();
    values.extend(session.0.values.values().cloned());
    values.extend(session.0.cookie_values());
    crate::scenario_runtime::check_binding_budget(
        &step.request,
        &session.0.values,
        config.budget.max_request_bytes as usize,
    )?;
    let bound = crate::authoring::bind_scenario_request(&step.request, &session.0.values)?;
    config = crate::scenario_runtime::request_config(&config, bound);
    let request = serde_json::to_value(&config)
        .map_err(|_| RunError::new("API_PREPARATION_FAILED", "Request encoding failed.", None))?;
    let plan = crate::ScenarioPlan {
        scenarios: vec![crate::Scenario {
            id: "api".into(),
            weight: 1,
            steps: vec![step.clone()],
        }],
        ..Default::default()
    };
    let base = config.clone();
    let validated = tokio::task::spawn_blocking(move || {
        let checked = crate::config::validate_request(config, None)?;
        crate::scenario_runtime::prepare(
            plan,
            &base,
            &checked.client,
            &checked.confirmation.origin,
        )?;
        Ok::<_, RunError>(checked)
    })
    .await
    .map_err(|error| {
        if error.is_panic() {
            std::panic::resume_unwind(error.into_panic());
        }
        RunError::new(
            "API_PREPARATION_FAILED",
            "Request preparation failed.",
            None,
        )
    })??;
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    let mut exchange = crate::scenario_runtime::Exchange {
        session: &mut session.0,
        step: &step,
        error: None,
    };
    let result = crate::transport::inspect(&validated, Some(&mut exchange), Some(&cancel.0)).await;
    let extraction_error = exchange.error.map(str::to_owned);
    if cancel.is_cancelled() {
        session.0.invalidate(extractions);
        return Err(cancelled());
    }
    let response = result.map_err(|failure| {
        session.0.invalidate(extractions);
        if let Some(code) = extraction_error.as_deref() {
            let message = if code == "INSECURE_AUTH_TARGET" {
                "Use HTTPS for session authentication. Cleartext cookies are allowed only on literal loopback IP addresses for local testing."
            } else {
                "The request could not safely apply its session cookie state."
            };
            return RunError::new(code, message, None);
        }
        match failure {
            crate::transport::Failure::Timeout => {
                RunError::new("API_TIMEOUT", "The request exceeded its timeout.", None)
            }
            failure => {
                let (category, _) = failure.diagnostic();
                RunError::new(
                    &format!("API_TRANSPORT_{}", category.to_ascii_uppercase()),
                    "The request failed. Check connectivity, TLS, redirects, and uploads.",
                    None,
                )
            }
        }
    })?;
    values.extend(session.0.values.values().cloned());
    values.extend(session.0.cookie_values());
    let response = crate::preview::redact_response(response, &validated, &request, &values)?;
    Ok((response, extraction_error))
}

fn cancelled() -> RunError {
    RunError::new("API_CANCELLED", "The API request was cancelled.", None)
}

/// Validates a functional request and selectors without traffic or session values.
///
/// # Arguments
/// * `config` - Environment-resolved request; session references are checked with bounded placeholders.
/// * `extractions` - Selectors applied after the response.
///
/// # Returns
/// No value. Uploads are opened for validation and closed before returning.
///
/// # Errors
/// Returns shared request, extraction, upload, or resource-budget validation errors.
///
/// # Examples
/// ```
/// let config = velo_engine::RunConfig { url: "https://example.com".into(), ..Default::default() };
/// velo_engine::validate_api_request(&config, &[])?;
/// # Ok::<(), velo_engine::RunError>(())
/// ```
pub fn validate_api_request(
    config: &RunConfig,
    extractions: &[Extraction],
) -> Result<(), RunError> {
    let mut base = config.clone();
    if base.scenarios.is_some() || base.data.is_some() {
        return Err(RunError::new(
            "API_REQUEST_REQUIRED",
            "Select a single HTTP request.",
            None,
        ));
    }
    base.workload = crate::Workload::Constant {
        vus: 1,
        duration_ms: 1000,
    };
    base.thresholds.clear();
    base.abort_policies.clear();
    base.warmup_ms = 0;
    base.pacing_ms = 0;
    let step = ScenarioStep {
        id: "request".into(),
        request: crate::ScenarioRequest {
            url: base.url.clone(),
            method: base.method,
            headers: base.headers.clone(),
            query: base.query.clone(),
            body: base.body.clone(),
            auth: base.auth.clone(),
            assertions: base.assertions.clone(),
        },
        extractions: extractions.to_vec(),
        think_ms: 0,
        on_failure: crate::StepFailure::Stop,
    };
    let values = crate::scenario_runtime::placeholder_values(&step.request)?;
    let bound = crate::authoring::bind_scenario_request(&step.request, &values)?;
    let checked = crate::config::validate_request(
        crate::scenario_runtime::request_config(&base, bound),
        None,
    )?;
    let plan = crate::ScenarioPlan {
        scenarios: vec![crate::Scenario {
            id: "api".into(),
            weight: 1,
            steps: vec![step],
        }],
        ..Default::default()
    };
    crate::scenario_runtime::prepare(plan, &base, &checked.client, &checked.confirmation.origin)?;
    Ok(())
}

/// Allocates functional dataset rows using the shared deterministic engine allocator.
///
/// # Arguments
/// * `definition` - Validated dataset recipe, including allocation seed and exhaustion policy.
/// * `records` - Number of prepared records, between one and 10,000.
/// * `count` - Maximum allocations, bounded to 1,000 functional steps.
///
/// # Returns
/// Ordered row indexes. Stop-on-exhaustion returns a shorter vector; repeat cycles reuse the same order.
///
/// # Errors
/// Returns `INVALID_DATASET` for invalid bounds or `DATASET_EXHAUSTED` with fail-on-exhaustion.
///
/// # Examples
/// ```
/// # fn example(data: &velo_engine::DataDefinition) -> Result<(), velo_engine::RunError> {
/// let indexes = velo_engine::api_row_indexes(data, 10, 1)?;
/// assert_eq!(indexes.len(), 1);
/// # Ok(()) }
/// ```
pub fn api_row_indexes(
    definition: &crate::DataDefinition,
    records: usize,
    count: usize,
) -> Result<Vec<usize>, RunError> {
    definition.validate()?;
    if !(1..=10_000).contains(&records) || count > 1_000 {
        return Err(RunError::new(
            "INVALID_DATASET",
            "Prepare 1 to 10,000 records and at most 1,000 allocations.",
            Some("dataset"),
        ));
    }
    let mut allocator = crate::dataset::Allocator::new(
        definition.allocation,
        definition.exhaustion,
        definition.seed,
        records,
    );
    let mut summary = definition.summary(records);
    let mut indexes = Vec::with_capacity(count);
    let allocations = if definition.lifetime == crate::DataLifetime::Iteration {
        count
    } else {
        count.min(1)
    };
    for _ in 0..allocations {
        if let Some(index) = allocator.take(&mut summary) {
            indexes.push(index);
        } else if definition.exhaustion == crate::DataExhaustion::Fail {
            return Err(RunError::new(
                "DATASET_EXHAUSTED",
                "Prepare enough records for this functional run.",
                Some("dataset"),
            ));
        } else {
            break;
        }
    }
    if definition.lifetime != crate::DataLifetime::Iteration
        && let Some(first) = indexes.first().copied()
    {
        indexes.resize(count, first);
    }
    Ok(indexes)
}

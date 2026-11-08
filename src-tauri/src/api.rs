//! Desktop admission, sequential orchestration, and isolated interactive API sessions.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};

use tauri::Emitter;
use velo_engine::{
    ApiCancellation, ApiCollection, ApiSession, ApiSessionView, CheckOutcome, CollectionRequest,
    FunctionalResult, FunctionalRun, FunctionalState, RunConfig, RunError, RunState,
    TestDefinition,
};

use crate::{execution::DesktopRuns, history::History};

#[derive(Default)]
pub(crate) struct ApiState {
    active: Mutex<Option<(String, ApiCancellation)>>,
    sessions: tokio::sync::Mutex<BTreeMap<(String, Option<String>), ApiSession>>,
    dirty: std::sync::atomic::AtomicBool,
}

struct Operation<'a>(&'a ApiState);

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.0.active.lock()
            && let Some((_, cancellation)) = active.take()
        {
            cancellation.cancel();
        }
    }
}

pub(crate) fn failure(code: &str, message: &str) -> RunError {
    RunError {
        code: code.into(),
        message: message.into(),
        field: None,
    }
}

impl ApiState {
    pub(crate) fn cancel(&self) {
        if let Ok(active) = self.active.lock()
            && let Some((_, cancel)) = active.as_ref()
        {
            cancel.cancel();
        }
    }

    pub(crate) async fn clear_collection(&self, id: &str) {
        self.sessions
            .lock()
            .await
            .retain(|(collection_id, _), _| collection_id != id);
    }

    pub(crate) fn dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }
}

#[tauri::command]
pub(crate) fn api_draft_state(state: tauri::State<'_, ApiState>, dirty: bool) {
    state.dirty.store(dirty, Ordering::Release);
}

#[tauri::command]
pub(crate) fn cancel_api_run(
    state: tauri::State<'_, ApiState>,
    operation_id: String,
) -> Result<(), RunError> {
    let active = state
        .active
        .lock()
        .map_err(|_| failure("API_STATE_UNAVAILABLE", "API state is unavailable."))?;
    if let Some((id, cancel)) = active.as_ref()
        && id == &operation_id
    {
        cancel.cancel();
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn api_session(
    state: tauri::State<'_, ApiState>,
    collection_id: String,
    environment: Option<String>,
) -> Result<ApiSessionView, RunError> {
    Ok(state
        .sessions
        .lock()
        .await
        .get(&(collection_id, environment))
        .map_or_else(ApiSessionView::default, ApiSession::view))
}

#[tauri::command]
pub(crate) async fn clear_api_session(
    state: tauri::State<'_, ApiState>,
    collection_id: String,
    environment: Option<String>,
) -> Result<(), RunError> {
    if state
        .active
        .lock()
        .map_err(|_| failure("API_STATE_UNAVAILABLE", "API state is unavailable."))?
        .is_some()
    {
        return Err(failure(
            "API_BUSY",
            "Cancel the API operation before clearing its session.",
        ));
    }
    state
        .sessions
        .lock()
        .await
        .remove(&(collection_id, environment));
    Ok(())
}

async fn resolve_request(
    history: &History,
    definition: TestDefinition,
    environment: Option<String>,
    row: BTreeMap<String, String>,
) -> Result<(RunConfig, Vec<String>), RunError> {
    history
        .configuration(true, move |_| {
            let mut saved = BTreeMap::new();
            let mut secrets = BTreeMap::new();
            let config = definition.resolve_api(environment.as_deref(), &row, |name| {
                if let Some(value) = secrets.get(name) {
                    return Ok(String::clone(value));
                }
                let value = crate::authoring::read_secret(name, &mut saved)?;
                secrets.insert(name.to_owned(), value.clone());
                Ok(value)
            })?;
            Ok((config, secrets.into_values().collect()))
        })
        .await
}

#[tauri::command]
pub(crate) async fn preview_api_request(
    history: tauri::State<'_, History>,
    collection: ApiCollection,
    request_id: String,
    environment: Option<String>,
    rows: Option<Vec<BTreeMap<String, String>>>,
) -> Result<serde_json::Value, RunError> {
    let request = collection.request(&request_id)?;
    let row = rows
        .unwrap_or_default()
        .into_iter()
        .next()
        .unwrap_or_default();
    let (config, secrets) = resolve_request(&history, request.definition, environment, row).await?;
    let mut value = serde_json::to_value(&config)
        .map_err(|_| failure("API_PREPARATION_FAILED", "Request encoding failed."))?;
    mask_preview(&mut value, &secrets);
    Ok(value)
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri command arguments mirror explicit runner controls"
)]
pub(crate) async fn run_api_collection<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, ApiState>,
    runs: tauri::State<'_, Arc<DesktopRuns>>,
    history: tauri::State<'_, History>,
    operation_id: String,
    collection: ApiCollection,
    selection: Vec<String>,
    environment: Option<String>,
    iterations: u32,
    delay_ms: u32,
    stop_on_failure: bool,
    authorized: bool,
    interactive: bool,
    rows: Vec<BTreeMap<String, String>>,
) -> Result<FunctionalRun, RunError> {
    if !authorized {
        return Err(failure(
            "AUTHORIZATION_REQUIRED",
            "Confirm you are authorized to send these API requests.",
        ));
    }
    if uuid::Uuid::parse_str(&operation_id).is_err()
        || !(1..=100).contains(&iterations)
        || delay_ms > 60_000
        || rows.len() > 10_000
    {
        return Err(failure(
            "INVALID_API_RUN",
            "Use a valid operation ID, 1 to 100 iterations, delay up to 60 seconds, and at most 10,000 dataset rows.",
        ));
    }
    rows.iter()
        .flat_map(|row| row.iter())
        .try_fold(0usize, |bytes, (name, value)| {
            let bytes = bytes.saturating_add(name.len()).saturating_add(value.len());
            if bytes > 8_388_608 {
                Err(failure(
                    "API_DATASET_BUDGET_EXCEEDED",
                    "Prepared dataset rows exceed the 8 MiB limit.",
                ))
            } else {
                Ok(bytes)
            }
        })?;
    let _admission = runs
        .admission
        .try_lock()
        .map_err(|_| failure("REQUEST_BUSY", "Another request operation is active."))?;
    if runs.closing.load(Ordering::Acquire) {
        return Err(failure("APP_EXITING", "The application is shutting down."));
    }
    let cancellation = ApiCancellation::default();
    {
        let mut active = state
            .active
            .lock()
            .map_err(|_| failure("API_STATE_UNAVAILABLE", "API state is unavailable."))?;
        *active = Some((operation_id.clone(), cancellation.clone()));
    }
    let _operation = Operation(&state);
    if runs.manager.current().await.is_some_and(|snapshot| {
        matches!(
            snapshot.state,
            RunState::Starting | RunState::Running | RunState::Stopping
        )
    }) {
        return Err(failure(
            "RUN_ALREADY_ACTIVE",
            "Stop the load test before sending API requests.",
        ));
    }
    let requests = collection.requests(&selection)?;
    let total = requests.len().saturating_mul(iterations as usize);
    if total == 0 || total > 1_000 || (interactive && (total != 1 || iterations != 1)) {
        return Err(failure(
            "API_RUN_LIMIT",
            "Select 1 to 1,000 total steps. Interactive sends require exactly one request.",
        ));
    }
    let key = (collection.id.clone(), environment.clone());
    let mut session = if interactive {
        let mut sessions = state.sessions.lock().await;
        if sessions.len() >= 32 && !sessions.contains_key(&key) {
            return Err(failure(
                "API_SESSION_LIMIT",
                "Clear an existing interactive session before using another collection and environment. The limit is 32 sessions.",
            ));
        }
        sessions.remove(&key).unwrap_or_default()
    } else {
        ApiSession::default()
    };
    let mut report = FunctionalRun {
        kind: velo_engine::FunctionalKind::Functional,
        id: operation_id,
        collection_id: collection.id.clone(),
        environment: environment.clone(),
        state: velo_engine::FunctionalRunState::Running,
        total: total as u32,
        requests: collection.portable()?.requests(&selection)?,
        results: Vec::with_capacity(total),
        session: session.view(),
        storage_error: None,
    };
    let result = {
        let work = execute(
            &app,
            &history,
            &requests,
            &environment,
            iterations,
            delay_ms,
            stop_on_failure,
            &rows,
            &collection.sensitive_response_fields,
            &mut session,
            &cancellation,
            &mut report,
        );
        tokio::pin!(work);
        tokio::select! {
            biased;
            _ = runs.preview_exit.notified() => { cancellation.cancel(); work.await },
            result = &mut work => result,
        }
    };
    if interactive {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(key, session);
    }
    result?;
    report.state = if cancellation.is_cancelled() {
        velo_engine::FunctionalRunState::Cancelled
    } else {
        velo_engine::FunctionalRunState::Completed
    };
    let persisted = report.clone();
    if let Err(error) = history
        .call(move |store| store.save_api_run(&persisted))
        .await
    {
        report.storage_error = Some(error.code);
    }
    let _ = app.emit_to("main", "api-progress", &report);
    Ok(report)
}

#[expect(
    clippy::too_many_arguments,
    reason = "One sequential operation uses explicit context and runner controls"
)]
async fn execute<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    history: &History,
    requests: &[CollectionRequest],
    environment: &Option<String>,
    iterations: u32,
    delay_ms: u32,
    stop_on_failure: bool,
    rows: &[BTreeMap<String, String>],
    sensitive_fields: &[String],
    session: &mut ApiSession,
    cancel: &ApiCancellation,
    report: &mut FunctionalRun,
) -> Result<(), RunError> {
    let mut prepared = Vec::with_capacity(report.total as usize);
    let mut bytes = 0usize;
    let mut allocations = Vec::with_capacity(requests.len());
    for request in requests {
        allocations.push(match &request.definition.dataset {
            Some(data) => Some(velo_engine::api_row_indexes(
                data,
                rows.len(),
                iterations as usize,
            )?),
            None => None,
        });
    }
    for iteration in 0..iterations {
        for (index, request) in requests.iter().enumerate() {
            if cancel.is_cancelled() {
                break;
            }
            let row = match &allocations[index] {
                None => BTreeMap::new(),
                Some(indexes) => {
                    let Some(index) = indexes.get(iteration as usize) else {
                        prepared.push(None);
                        continue;
                    };
                    rows.get(*index).cloned().ok_or_else(|| {
                        failure(
                            "API_DATASET_REQUIRED",
                            "Prepare dataset records before running this collection.",
                        )
                    })?
                }
            };
            let (config, secrets) = resolve_request(
                history,
                request.definition.clone(),
                environment.clone(),
                row,
            )
            .await?;
            bytes = bytes.saturating_add(
                serde_json::to_vec(&config)
                    .map_err(|_| failure("API_PREPARATION_FAILED", "Request encoding failed."))?
                    .len(),
            );
            if bytes > 16_777_216 {
                return Err(failure(
                    "API_RUN_BUDGET_EXCEEDED",
                    "Resolved collection requests exceed the 16 MiB limit.",
                ));
            }
            let config_check = config.clone();
            let extractions = request.extractions.clone();
            tokio::task::spawn_blocking(move || {
                velo_engine::validate_api_request(&config_check, &extractions)
            })
            .await
            .map_err(|error| {
                if error.is_panic() {
                    std::panic::resume_unwind(error.into_panic());
                }
                failure("API_PREPARATION_FAILED", "Request validation failed.")
            })??;
            prepared.push(Some((config, secrets)));
        }
    }
    let mut prepared = prepared.into_iter();
    let mut stopped = false;
    let mut evidence_bytes = 0usize;
    for iteration in 1..=iterations {
        for request in requests {
            let mut result = if stopped || cancel.is_cancelled() {
                FunctionalResult {
                    request_id: request.id.clone(),
                    name: request.name.clone(),
                    iteration,
                    state: FunctionalState::Skipped,
                    response: None,
                    error: None,
                }
            } else if let Some(Some((config, secrets))) = prepared.next() {
                let outcome = velo_engine::send_api_request(
                    config,
                    &request.extractions,
                    &secrets,
                    session,
                    cancel,
                )
                .await;
                match outcome {
                    Ok((mut response, extraction_error)) => {
                        velo_engine::redact_response_fields(&mut response, sensitive_fields);
                        let expected_status =
                            request
                                .definition
                                .config
                                .assertions
                                .iter()
                                .any(|assertion| {
                                    matches!(
                                        assertion.rule,
                                        velo_engine::AssertionRule::Status { .. }
                                    )
                                });
                        let passed = ((200..400).contains(&response.status) || expected_status)
                            && response
                                .checks
                                .iter()
                                .all(|check| *check == CheckOutcome::Passed)
                            && extraction_error.is_none();
                        FunctionalResult {
                            request_id: request.id.clone(),
                            name: request.name.clone(),
                            iteration,
                            state: if passed {
                                FunctionalState::Passed
                            } else {
                                FunctionalState::Failed
                            },
                            response: Some(response),
                            error: extraction_error,
                        }
                    }
                    Err(error) => FunctionalResult {
                        request_id: request.id.clone(),
                        name: request.name.clone(),
                        iteration,
                        state: if cancel.is_cancelled() {
                            FunctionalState::Cancelled
                        } else {
                            FunctionalState::Failed
                        },
                        response: None,
                        error: Some(error.code),
                    },
                }
            } else {
                stopped = true;
                FunctionalResult {
                    request_id: request.id.clone(),
                    name: request.name.clone(),
                    iteration,
                    state: FunctionalState::Skipped,
                    response: None,
                    error: Some("DATASET_EXHAUSTED".into()),
                }
            };
            if let Some(response) = &mut result.response {
                evidence_bytes = evidence_bytes.saturating_add(
                    response.body.len()
                        + response
                            .headers
                            .iter()
                            .map(|pair| pair.name.len() + pair.value.len())
                            .sum::<usize>(),
                );
                if evidence_bytes > 1_048_576 {
                    response.body =
                        "[Response content withheld: functional run evidence limit reached.]"
                            .into();
                    response.headers.clear();
                    response.truncated = true;
                    response.redacted = true;
                }
            }
            stopped |= stop_on_failure && result.state == FunctionalState::Failed;
            report.results.push(result);
            report.session = session.view();
            let progress = FunctionalRun {
                kind: report.kind,
                id: report.id.clone(),
                collection_id: report.collection_id.clone(),
                environment: report.environment.clone(),
                state: report.state,
                total: report.total,
                requests: Vec::new(),
                results: report
                    .results
                    .iter()
                    .map(|result| FunctionalResult {
                        request_id: result.request_id.clone(),
                        name: result.name.clone(),
                        iteration: result.iteration,
                        state: result.state,
                        error: result.error.clone(),
                        response: result.response.as_ref().map(|response| {
                            velo_engine::PreviewResponse {
                                status: response.status,
                                elapsed_ms: response.elapsed_ms,
                                checks: response.checks.clone(),
                                received_bytes: response.received_bytes,
                                body_limit: response.body_limit,
                                body_format: response.body_format,
                                content_retained: false,
                                truncated: response.truncated,
                                redacted: true,
                                ..Default::default()
                            }
                        }),
                    })
                    .collect(),
                session: report.session.clone(),
                storage_error: None,
            };
            let _ = app.emit_to("main", "api-progress", &progress);
            if !stopped
                && !cancel.is_cancelled()
                && report.results.len() < report.total as usize
                && delay_ms > 0
            {
                tokio::select! { _ = cancel.cancelled() => {}, _ = tokio::time::sleep(Duration::from_millis(delay_ms.into())) => {} }
            }
        }
    }
    Ok(())
}

fn mask_preview(value: &mut serde_json::Value, secrets: &[String]) {
    match value {
        serde_json::Value::String(text) => {
            for secret in secrets {
                if !secret.is_empty() {
                    *text = text.replace(secret, "[redacted]");
                }
            }
            if let Ok(mut url) = reqwest::Url::parse(text) {
                if !url.username().is_empty() || url.password().is_some() {
                    *text = "[redacted URL credentials]".into();
                } else if url.query().is_some() {
                    url.set_query(Some("[redacted]"));
                    *text = url.to_string();
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                mask_preview(item, secrets);
            }
        }
        serde_json::Value::Object(items) => {
            let sensitive_pair = items
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(sensitive_name);
            for (key, item) in items {
                if sensitive_name(key) || key == "path" || (sensitive_pair && key == "value") {
                    *item = serde_json::Value::String("[redacted]".into());
                } else {
                    mask_preview(item, secrets);
                }
            }
        }
        _ => {}
    }
}

fn sensitive_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase().replace(['-', '_'], "");
    [
        "authorization",
        "proxyauthorization",
        "cookie",
        "setcookie",
        "password",
        "secret",
        "token",
        "apikey",
    ]
    .iter()
    .any(|field| name.contains(field))
}

#[cfg(test)]
mod tests;

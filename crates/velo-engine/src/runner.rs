mod scenarios;

use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use serde::Serialize;
use tokio::{
    sync::Mutex as AsyncMutex,
    task::{JoinError, JoinHandle, JoinSet},
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::{
    config::{RunConfig, RunConfirmation, RunError, ValidatedConfig, validate},
    metrics::{LatencySnapshot, Metrics, MetricsSnapshot, Outcome, decimal},
};

/// Native execution lifecycle. Terminal states are published only after worker cleanup.
///
/// # Examples
/// ```
/// assert_eq!(serde_json::to_value(velo_engine::RunState::Cancelled)?, "cancelled");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// Accepted, with the supervisor waiting to begin execution.
    Starting,
    /// Accepting new sequential iterations.
    Running,
    /// Admission closed; draining or cancelling owned requests.
    Stopping,
    /// Scheduling duration expired and all tasks were joined.
    Completed,
    /// Explicit stop requested and all tasks were joined; partial results retained.
    Cancelled,
    /// An owned task failed; remaining work was cleaned up.
    Failed,
}

impl RunState {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}

/// Authoritative current state or final summary. No request secrets are included.
///
/// # Examples
/// A cancelled final summary retains measurements but represents partial work:
/// ```
/// fn is_partial(summary: &velo_engine::RunSnapshot) -> bool {
///     summary.state == velo_engine::RunState::Cancelled
/// }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct RunSnapshot {
    /// Separate scenario iterations, fixed step populations and lifecycle outcomes.
    pub scenarios: Option<crate::ScenarioReport>,
    /// Monotonically increasing application-instance identifier, encoded as decimal.
    pub run_id: String,
    /// Lifecycle state, terminal only after cleanup.
    pub state: RunState,
    /// Safe target and workload details.
    pub confirmation: RunConfirmation,
    /// Scheduling time observed so far, in microseconds as a decimal string.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub scheduling_us: u64,
    /// Time since admission closed, in microseconds as a decimal string.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub drain_us: u64,
    /// Scheduling plus drain duration, in microseconds as a decimal string.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub actual_us: u64,
    /// Desired workers at the last scheduler reconciliation; zero during drain.
    pub desired_vus: u32,
    /// Workers finishing their current request before retirement.
    pub retiring_vus: u32,
    /// Currently owned VU workers; zero after cleanup.
    pub active_vus: u32,
    /// Maximum concurrently owned VU workers.
    pub max_vus: u32,
    /// Arrival scheduling counters; zero for VU workloads. Drops exclude HTTP failures.
    pub arrival: ArrivalSnapshot,
    /// Cumulative counters, rates, and separate duration populations.
    pub metrics: MetricsSnapshot,
    /// Final threshold outcomes; empty while active or when no rules are configured.
    #[ts(type = "ThresholdResult[]")]
    pub thresholds: Vec<crate::ThresholdResult>,
    /// Stable internal failure code, without infrastructure or target details.
    pub failure_code: Option<String>,
    /// Safe preparation metadata and final allocation counts; never includes rows.
    pub dataset: Option<crate::DataSummary>,
}

/// One sequenced aggregate interval and the authoritative cumulative state.
///
/// Interval response latency excludes transport failures, timeouts, and cancellations.
/// Lost updates cannot be reconstructed; cumulative totals remain recoverable through
/// [`RunManager::current`]. No individual observations or request secrets are retained.
///
/// # Examples
/// ```
/// fn requests_per_second(update: &velo_engine::RunUpdate) -> Option<f64> {
///     (update.interval_us > 0).then(|| update.interval_requests as f64
///         * 1_000_000.0 / update.interval_us as f64)
/// }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct RunUpdate {
    /// Sequence starting at one within each run, encoded as decimal.
    pub sequence: String,
    /// Authoritative cumulative state at this sample.
    pub snapshot: RunSnapshot,
    /// Monotonic time since the preceding sample or execution start, in microseconds.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub interval_us: u64,
    /// Completed non-cancelled attempts in this interval.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub interval_requests: u64,
    /// Status, transport, and timeout failures in this interval.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub interval_failures: u64,
    /// Bounded response-latency population for this interval.
    pub response_latency: LatencySnapshot,
}

/// Arrival admission accounting, independent of HTTP request outcomes.
///
/// After cleanup, offered equals started iterations plus drop reasons. Counters
/// are decimal strings on IPC. No pending iteration queue or per-slot history exists.
///
/// # Examples
/// ```
/// let scheduling = velo_engine::ArrivalSnapshot::default();
/// assert_eq!(scheduling.offered, 0);
/// ```
#[derive(Clone, Debug, Default, Serialize, TS)]
pub struct ArrivalSnapshot {
    /// Deadlines reached while scheduling was open, including dropped iterations.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub offered: u64,
    /// Current slots rejected because all concurrency capacity was owned.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub capacity_drops: u64,
    /// Slots rejected because a stop or fail dataset exhaustion policy closed admission.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub dataset_drops: u64,
    /// Expired slots skipped after a delayed scheduler wake; never replayed.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub missed_deadlines: u64,
    /// Maximum wake delay against the next unprocessed deadline, in microseconds.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub max_lateness_us: u64,
}

struct Progress {
    scenarios: Option<crate::scenario_metrics::ScenarioMetrics>,
    state: RunState,
    metrics: Metrics,
    arrival: ArrivalSnapshot,
    sequence: u64,
    sampled_us: u64,
    started: Option<Instant>,
    deadline: Option<Instant>,
    scheduling_end: Option<Instant>,
    finished: Option<Instant>,
    load_finished: Option<Instant>,
    desired_vus: u32,
    retiring_vus: u32,
    active_vus: u32,
    max_vus: u32,
    failure_code: Option<String>,
    dataset: Option<crate::DataSummary>,
    allocator: Option<crate::dataset::Allocator>,
    run_record: Option<usize>,
}

struct Run {
    id: String,
    confirmation: RunConfirmation,
    thresholds: Vec<crate::Threshold>,
    stop: CancellationToken,
    progress: Mutex<Progress>,
    diagnostics: crate::transport::FailureDiagnostics,
    setup_session: Mutex<crate::scenario_runtime::Session>,
}

impl Run {
    fn lock(&self) -> MutexGuard<'_, Progress> {
        self.progress
            .lock()
            .expect("run progress was poisoned by a programming panic")
    }

    fn measured_lock(&self) -> MutexGuard<'_, Progress> {
        let waiting = Instant::now();
        let mut progress = self.lock();
        progress.metrics.wait(waiting.elapsed());
        progress
    }

    fn snapshot(&self) -> RunSnapshot {
        self.snapshot_locked(&self.lock())
    }

    fn snapshot_locked(&self, progress: &Progress) -> RunSnapshot {
        let now = progress
            .load_finished
            .or(progress.finished)
            .unwrap_or_else(Instant::now);
        let start = progress.started.unwrap_or(now);
        let end = progress.scheduling_end.unwrap_or(now);
        let elapsed = now.saturating_duration_since(start);
        let scheduling_us = end.saturating_duration_since(start).as_micros() as u64;
        let actual_us = elapsed.as_micros() as u64;
        let metrics = progress.metrics.snapshot(elapsed);
        let thresholds = if progress.state.is_terminal() {
            crate::threshold::evaluate(&self.thresholds, &metrics)
        } else {
            Vec::new()
        };
        RunSnapshot {
            scenarios: progress
                .scenarios
                .as_ref()
                .map(crate::scenario_metrics::ScenarioMetrics::snapshot),
            thresholds,
            run_id: self.id.clone(),
            state: progress.state,
            confirmation: self.confirmation.clone(),
            scheduling_us,
            drain_us: actual_us - scheduling_us,
            actual_us,
            desired_vus: progress.desired_vus,
            retiring_vus: progress.retiring_vus,
            active_vus: progress.active_vus,
            max_vus: progress.max_vus,
            metrics,
            arrival: progress.arrival.clone(),
            failure_code: progress.failure_code.clone(),
            dataset: progress.dataset.clone(),
        }
    }

    fn stop(&self) {
        let mut progress = self.lock();
        if !progress.state.is_terminal() {
            progress.state = RunState::Stopping;
            progress.desired_vus = 0;
            let now = Instant::now();
            let end = progress.deadline.map_or(now, |deadline| now.min(deadline));
            progress.scheduling_end.get_or_insert(end);
            self.stop.cancel();
        }
    }
}

struct OwnedRun {
    run: Arc<Run>,
    task: Option<JoinHandle<()>>,
}

impl Drop for OwnedRun {
    fn drop(&mut self) {
        self.run.stop();
    }
}

#[derive(Default)]
struct ManagerState {
    last_id: u64,
    current: Option<OwnedRun>,
}

/// Owns at most one run and retains its final summary until the next accepted start.
///
/// Use from a Tokio runtime. Call [`Self::shutdown`] before runtime teardown to
/// stop and join native work. Dropping the manager signals stop; only awaiting
/// shutdown guarantees cleanup has completed before the caller proceeds. Poisoned
/// run state is never reused after a programming panic.
///
/// # Examples
/// ```no_run
/// # async fn example() -> Result<(), velo_engine::RunError> {
/// use velo_engine::{RunConfig, RunManager};
/// let manager = RunManager::default();
/// let config = RunConfig { url: "http://127.0.0.1:8080/health".into(), ..RunConfig::default() };
/// let confirmation = RunManager::prepare(config.clone())?;
/// assert_eq!(confirmation.vus, 1);
/// let started = manager.start(config, true).await?;
/// manager.stop(&started.run_id).await?;
/// manager.shutdown().await;
/// assert_eq!(manager.current().await.unwrap().active_vus, 0);
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct RunManager {
    state: AsyncMutex<ManagerState>,
}

impl std::fmt::Debug for RunManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("RunManager").finish_non_exhaustive()
    }
}

impl RunManager {
    /// Validates a configuration and returns safe confirmation details without HTTP traffic.
    ///
    /// # Arguments
    /// * `config` - Untrusted settings, consumed after validation.
    ///
    /// # Returns
    /// Target origin, workload, resource budgets, and high-load warning codes.
    ///
    /// # Errors
    /// Returns [`RunError`] with INVALID_RUN_CONFIG, VU_BUDGET_EXCEEDED,
    /// REQUEST_BUDGET_EXCEEDED, UPLOAD_FILE_UNAVAILABLE, or HTTP_CLIENT_INIT_FAILED
    /// on invalid input, unavailable files, or HTTP initialization failure.
    /// Dataset preparation also returns INVALID_DATASET or DATASET_BUDGET_EXCEEDED.
    /// Response checks can return INVALID_ASSERTION, INVALID_INSPECTION_LIMITS, or
    /// INSPECTION_BUDGET_EXCEEDED before allocating worker inspection buffers.
    /// Scenario validation also returns INVALID_SCENARIO, SCENARIO_BUDGET_EXCEEDED,
    /// or SESSION_BUDGET_EXCEEDED. Every setup, load and cleanup target must share
    /// the reviewed origin; session references cannot alter scheme or authority.
    /// All records are checked before traffic; every record must target the reviewed origin. Input
    /// secrets are omitted. Multipart preparation opens and closes files to validate
    /// readability; call this synchronous API from a blocking thread for file inputs.
    ///
    /// # Examples
    /// ```
    /// use velo_engine::{RunConfig, RunManager};
    /// let config = RunConfig { url: "https://example.com/health".into(), ..RunConfig::default() };
    /// assert_eq!(RunManager::prepare(config)?.origin, "https://example.com");
    /// # Ok::<(), velo_engine::RunError>(())
    /// ```
    pub fn prepare(config: RunConfig) -> Result<RunConfirmation, RunError> {
        Ok(validate(config)?.confirmation)
    }

    /// Validates and atomically starts one run, returning its ID before completion.
    ///
    /// # Arguments
    /// * `config` - Request, workload, and explicit local budgets.
    /// * `authorized` - Acknowledgement that the caller is authorized to load the
    ///   target at the requested load. This does not verify target ownership.
    ///
    /// # Returns
    /// The accepted run's initial authoritative snapshot. Only the most recent
    /// run is retained. Cancelling this future before acceptance starts no HTTP work.
    /// In-progress blocking file validation may finish afterward and closes its files.
    /// Owned streaming readers are cancelled and joined before terminal publication.
    ///
    /// # Errors
    /// Returns [`RunError`] with AUTHORIZATION_REQUIRED, RUN_ALREADY_ACTIVE,
    /// RUN_ID_EXHAUSTED, RUN_PREPARATION_FAILED if blocking preparation cannot finish,
    /// or validation/initialization codes from [`Self::prepare`].
    ///
    /// # Panics
    /// Panics if accepted outside a running Tokio runtime, if run progress was
    /// poisoned, or if request preparation or prior cleanup encountered a programming panic.
    ///
    /// # Examples
    /// ```no_run
    /// # async fn example() -> Result<(), velo_engine::RunError> {
    /// let manager = velo_engine::RunManager::default();
    /// let config = velo_engine::RunConfig { url: "http://127.0.0.1:8080/".into(), ..Default::default() };
    /// let snapshot = manager.start(config, true).await?;
    /// assert!(!snapshot.run_id.is_empty());
    /// manager.shutdown().await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(
        &self,
        config: RunConfig,
        authorized: bool,
    ) -> Result<RunSnapshot, RunError> {
        if !authorized {
            return Err(RunError::new(
                "AUTHORIZATION_REQUIRED",
                "Acknowledge authorization for this target and requested load.",
                None,
            ));
        }
        let config = if config.data.is_some()
            || config.scenarios.is_some()
            || matches!(&config.body, crate::RequestBody::Multipart(_))
        {
            tokio::task::spawn_blocking(move || validate(config))
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
                })??
        } else {
            validate(config)?
        };
        let mut state = self.state.lock().await;
        if let Some(current) = &mut state.current {
            if !current.run.lock().state.is_terminal() {
                return Err(RunError::new(
                    "RUN_ALREADY_ACTIVE",
                    "Stop the active run and wait for cleanup before starting another.",
                    None,
                ));
            }
            if let Some(task) = &mut current.task {
                let _ = task_failed(task.await);
            }
            current.task = None;
        }
        let id = state.last_id.checked_add(1).ok_or_else(|| {
            RunError::new(
                "RUN_ID_EXHAUSTED",
                "Run identifier capacity exhausted.",
                None,
            )
        })?;
        let run = Arc::new(Run {
            id: id.to_string(),
            confirmation: config.confirmation.clone(),
            thresholds: config.thresholds.clone(),
            stop: CancellationToken::new(),
            diagnostics: crate::transport::FailureDiagnostics::default(),
            setup_session: Mutex::default(),
            progress: Mutex::new(Progress {
                scenarios: config
                    .scenarios
                    .as_ref()
                    .map(|plan| crate::scenario_metrics::ScenarioMetrics::new(&plan.definition)),
                state: RunState::Starting,
                metrics: {
                    let mut metrics = Metrics::configured(
                        &config.assertions.definitions,
                        config.inspection.clone(),
                    );
                    if config.scenarios.is_some() {
                        metrics.scenarios();
                    }
                    metrics
                },
                arrival: ArrivalSnapshot::default(),
                sequence: 0,
                sampled_us: 0,
                started: None,
                deadline: None,
                scheduling_end: None,
                finished: None,
                load_finished: None,
                desired_vus: 0,
                retiring_vus: 0,
                active_vus: 0,
                max_vus: 0,
                dataset: config.confirmation.dataset.clone(),
                allocator: config.confirmation.dataset.as_ref().map(|summary| {
                    crate::dataset::Allocator::new(
                        summary.allocation,
                        summary.exhaustion,
                        summary.seed,
                        config.dataset_requests.len(),
                    )
                }),
                run_record: None,
                failure_code: None,
            }),
        });
        let snapshot = run.snapshot();
        let task = tokio::spawn(supervise(run.clone(), config));
        state.last_id = id;
        state.current = Some(OwnedRun {
            run,
            task: Some(task),
        });
        Ok(snapshot)
    }

    /// Reads the latest run, including immutable final measurements after cleanup.
    ///
    /// # Returns
    /// None before the first accepted start. Cancelling this read has no side effects.
    ///
    /// # Panics
    /// Panics if run progress was poisoned by a programming panic.
    ///
    /// # Examples
    /// ```
    /// # async fn example() {
    /// assert!(velo_engine::RunManager::default().current().await.is_none());
    /// # }
    /// ```
    pub async fn current(&self) -> Option<RunSnapshot> {
        self.state
            .lock()
            .await
            .current
            .as_ref()
            .map(|current| current.run.snapshot())
    }

    /// Samples and resets the current run's interval metrics without changing cumulative totals.
    ///
    /// Call from one aggregate publisher per run. Sampling holds a bounded metrics
    /// lock after HTTP latency measurement; contention is reported in cumulative metrics.
    ///
    /// # Arguments
    /// * `run_id` - Exact run ID, preventing a previous publisher from sampling a new run.
    ///
    /// # Returns
    /// The next sequenced interval, or None when the run is absent, replaced, or its
    /// sequence representation is exhausted. Cancelling before lock acquisition has
    /// no side effects. A successful sample consumes that interval's histogram.
    ///
    /// # Panics
    /// Panics if run progress was poisoned by a programming panic.
    ///
    /// # Examples
    /// ```
    /// # async fn example() {
    /// let manager = velo_engine::RunManager::default();
    /// assert!(manager.sample("missing").await.is_none());
    /// # }
    /// ```
    pub async fn sample(&self, run_id: &str) -> Option<RunUpdate> {
        let state = self.state.lock().await;
        let current = state.current.as_ref()?;
        if current.run.id != run_id {
            return None;
        }
        let mut progress = current.run.lock();
        let sequence = progress.sequence.checked_add(1)?;
        let snapshot = current.run.snapshot_locked(&progress);
        let interval_us = snapshot.actual_us.saturating_sub(progress.sampled_us);
        let (response_latency, interval_requests, interval_failures) =
            progress.metrics.take_interval();
        progress.sequence = sequence;
        progress.sampled_us = snapshot.actual_us;
        Some(RunUpdate {
            sequence: sequence.to_string(),
            snapshot,
            interval_us,
            interval_requests,
            interval_failures,
            response_latency,
        })
    }

    /// Closes admission for the matching run and requests graceful drain.
    ///
    /// # Arguments
    /// * `run_id` - Exact ID returned by start, preventing stale UI stop requests.
    ///
    /// # Returns
    /// Current snapshot. Repeated stop calls are safe, including after completion.
    /// This requests cleanup; use [`Self::current`] to observe terminal state.
    /// Once requested, stop continues even if the caller cancels this future.
    ///
    /// # Errors
    /// Returns [`RunError`] with RUN_NOT_FOUND if no matching current run exists.
    ///
    /// # Panics
    /// Panics if run progress was poisoned by a programming panic.
    ///
    /// # Examples
    /// ```
    /// # async fn example() {
    /// let error = velo_engine::RunManager::default().stop("missing").await.unwrap_err();
    /// assert_eq!(error.code, "RUN_NOT_FOUND");
    /// # }
    /// ```
    pub async fn stop(&self, run_id: &str) -> Result<RunSnapshot, RunError> {
        let state = self.state.lock().await;
        let current = state
            .current
            .as_ref()
            .filter(|current| current.run.id == run_id)
            .ok_or_else(|| {
                RunError::new(
                    "RUN_NOT_FOUND",
                    "The requested run is no longer current.",
                    None,
                )
            })?;
        current.run.stop();
        Ok(current.run.snapshot())
    }

    /// Stops the current run and waits until its supervisor has joined all workers.
    ///
    /// # Returns
    /// Completes after the configured grace and cancellation cleanup. Repeated
    /// calls are safe. Cancelling this future leaves stop requested; call again
    /// to finish awaiting cleanup before tearing down the runtime.
    ///
    /// # Panics
    /// Propagates programming panics from the supervisor or its workers. Panics if
    /// run progress was poisoned rather than treating partially changed state as valid.
    ///
    /// # Examples
    /// ```
    /// # async fn example() {
    /// velo_engine::RunManager::default().shutdown().await;
    /// # }
    /// ```
    pub async fn shutdown(&self) {
        let mut state = self.state.lock().await;
        if let Some(current) = &mut state.current {
            current.run.stop();
            if let Some(task) = &mut current.task {
                let _ = task_failed(task.await);
            }
            current.task = None;
        }
    }
}

fn task_failed(result: Result<(), JoinError>) -> bool {
    match result {
        Ok(()) => false,
        Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
        Err(error) => !error.is_cancelled(),
    }
}

async fn supervise(run: Arc<Run>, config: ValidatedConfig) {
    if let Some(plan) = &config.scenarios {
        let mut session = crate::scenario_runtime::Session::default();
        let outcome = scenarios::phase(
            &run,
            &config,
            plan,
            &mut session,
            crate::ScenarioPhase::Setup,
        )
        .await;
        *run.setup_session
            .lock()
            .expect("setup session poisoned by a programming panic") = session;
        if matches!(
            outcome,
            crate::PhaseOutcome::Failed | crate::PhaseOutcome::Cancelled
        ) {
            if outcome == crate::PhaseOutcome::Failed {
                run.lock().failure_code = Some("SETUP_FAILED".into());
            }
            run.stop.cancel();
        }
    }
    let started = Instant::now();
    let deadline = started + config.duration;
    {
        let mut progress = run.lock();
        progress.started = Some(started);
        progress.deadline = Some(deadline);
        if progress.state == RunState::Starting {
            progress.state = RunState::Running;
        } else {
            progress.scheduling_end = Some(started);
        }
    }
    let config = Arc::new(config);
    let mut workers = JoinSet::new();
    let mut controls: Vec<WorkerControl> = Vec::new();
    let mut failed = false;
    let arrival_rate = match config.confirmation.workload {
        crate::Workload::Arrival { rate, .. } => Some(rate),
        _ => None,
    };
    if let Some(rate) = arrival_rate.filter(|_| !run.stop.is_cancelled()) {
        failed = schedule_arrivals(&run, &config, &mut workers, started, rate).await;
    }
    while arrival_rate.is_none() {
        let now = Instant::now();
        if run.stop.is_cancelled() || now >= deadline {
            break;
        }
        let (desired, next) = config
            .schedule
            .position(now.saturating_duration_since(started));
        {
            let mut progress = run.lock();
            if progress.state != RunState::Running {
                break;
            }
            progress.desired_vus = desired;
            let accepting = controls
                .iter()
                .filter(|control| {
                    !control.retire.is_cancelled() && !control.finished.is_cancelled()
                })
                .count();
            let mut excess = accepting.saturating_sub(desired as usize);
            for control in controls.iter().rev() {
                if excess == 0 {
                    break;
                }
                if !control.retire.is_cancelled() && !control.finished.is_cancelled() {
                    control.retire.cancel();
                    progress.retiring_vus += 1;
                    excess -= 1;
                }
            }
        }
        let accepting = controls
            .iter()
            .filter(|control| !control.retire.is_cancelled() && !control.finished.is_cancelled())
            .count();
        let available = (config.confirmation.vus as usize).saturating_sub(controls.len());
        let additions = (desired as usize).saturating_sub(accepting).min(available);
        for _ in 0..additions.min(64) {
            let mut progress = run.lock();
            if progress.state != RunState::Running || Instant::now() >= deadline {
                break;
            }
            let retire = CancellationToken::new();
            let finished = CancellationToken::new();
            progress.active_vus += 1;
            progress.max_vus = progress.max_vus.max(progress.active_vus);
            let worker = Worker {
                run: run.clone(),
                retire: retire.clone(),
                finished: finished.clone(),
            };
            let handle = workers.spawn(execute_worker(worker, config.clone(), deadline));
            controls.push(WorkerControl {
                id: handle.id(),
                retire,
                finished,
            });
        }
        if additions > 64 {
            tokio::task::yield_now().await;
            continue;
        }
        tokio::select! {
            biased;
            _ = run.stop.cancelled() => break,
            _ = tokio::time::sleep_until(started + next) => {},
            result = workers.join_next_with_id(), if !workers.is_empty() => {
                match result {
                    Some(Ok((id, ()))) => {
                        if let Some(index) = controls.iter().position(|control| control.id == id) {
                            controls.swap_remove(index);
                        }
                    }
                    Some(Err(error)) => { failed = task_failed(Err(error)); break; }
                    None => {}
                }
            }
        }
    }
    let drain_start = {
        let mut progress = run.lock();
        progress.state = RunState::Stopping;
        progress.desired_vus = 0;
        let end = Instant::now().min(deadline);
        *progress.scheduling_end.get_or_insert(end)
    };
    let drain_deadline = drain_start + config.grace;
    while !workers.is_empty() {
        tokio::select! {
            biased;
            _ = tokio::time::sleep_until(drain_deadline) => {
                workers.abort_all();
                break;
            }
            result = workers.join_next() => {
                failed |= result.is_some_and(task_failed);
            }
        }
    }
    while let Some(result) = workers.join_next().await {
        failed |= task_failed(result);
    }
    run.lock().load_finished = Some(Instant::now());
    if let Some(plan) = &config.scenarios {
        let mut session = run
            .setup_session
            .lock()
            .expect("setup session poisoned by a programming panic")
            .clone();
        let outcome = scenarios::phase(
            &run,
            &config,
            plan,
            &mut session,
            crate::ScenarioPhase::Cleanup,
        )
        .await;
        if matches!(
            outcome,
            crate::PhaseOutcome::Failed | crate::PhaseOutcome::Cancelled
        ) {
            run.lock()
                .failure_code
                .get_or_insert_with(|| "CLEANUP_FAILED".into());
        }
    }
    config.upload_stop.cancel();
    let mut uploads = config.uploads.lock().await;
    while let Some(result) = uploads.join_next().await {
        failed |= task_failed(result);
    }
    drop(uploads);
    let mut progress = run.lock();
    progress.allocator = None;
    progress.run_record = None;
    progress.finished = Some(Instant::now());
    progress.state = if failed {
        progress.failure_code = Some("WORKER_FAILED".into());
        RunState::Failed
    } else if progress.failure_code.is_some() {
        RunState::Failed
    } else if progress.dataset.as_ref().is_some_and(|data| data.exhausted) {
        RunState::Completed
    } else if run.stop.is_cancelled() {
        RunState::Cancelled
    } else {
        RunState::Completed
    };
}

fn arrival_due(elapsed: Duration, rate: u32, duration: Duration) -> u64 {
    let total = (duration.as_nanos() * u128::from(rate)).div_ceil(1_000_000_000);
    let reached = elapsed.as_nanos() * u128::from(rate) / 1_000_000_000 + 1;
    reached.min(total) as u64
}

fn arrival_offset(index: u64, rate: u32) -> Duration {
    Duration::from_nanos((u128::from(index) * 1_000_000_000).div_ceil(u128::from(rate)) as u64)
}

async fn schedule_arrivals(
    run: &Arc<Run>,
    config: &Arc<ValidatedConfig>,
    workers: &mut JoinSet<()>,
    started: Instant,
    rate: u32,
) -> bool {
    let deadline = started + config.duration;
    let mut processed = 0;
    loop {
        while let Some(result) = workers.try_join_next() {
            if task_failed(result) {
                return true;
            }
        }
        let next = started + arrival_offset(processed, rate);
        tokio::select! {
            biased;
            _ = run.stop.cancelled() => break,
            _ = tokio::time::sleep_until(next.min(deadline)) => {},
            result = workers.join_next(), if !workers.is_empty() => {
                if result.is_some_and(task_failed) {
                    return true;
                }
                continue;
            }
        }
        let mut progress = run.measured_lock();
        let now = Instant::now();
        let end = progress.scheduling_end.unwrap_or(now).min(deadline);
        let due = arrival_due(
            end.saturating_duration_since(started),
            rate,
            config.duration,
        );
        let count = due.saturating_sub(processed);
        if count == 0 {
            if progress.state != RunState::Running || now >= deadline {
                break;
            }
            continue;
        }
        progress.arrival.offered += count;
        progress.arrival.max_lateness_us = progress
            .arrival
            .max_lateness_us
            .max(now.saturating_duration_since(next).as_micros() as u64);
        processed = due;
        if progress.state != RunState::Running || now >= deadline {
            progress.arrival.missed_deadlines += count;
            break;
        }
        progress.desired_vus = config.confirmation.vus;
        progress.arrival.missed_deadlines += count - 1;
        if workers.len() >= config.confirmation.vus as usize {
            progress.arrival.capacity_drops += 1;
            continue;
        }
        let Some(record) = select_record(run, &mut progress, &mut None) else {
            progress.arrival.dataset_drops += 1;
            break;
        };
        let scenario = scenarios::admit(&mut progress, config.scenarios.as_ref());
        if scenario.is_none() {
            progress.metrics.admission();
        }
        progress.active_vus += 1;
        progress.max_vus = progress.max_vus.max(progress.active_vus);
        let worker = Worker {
            run: run.clone(),
            retire: CancellationToken::new(),
            finished: CancellationToken::new(),
        };
        if let Some(scenario) = scenario {
            workers.spawn(scenarios::arrival(worker, config.clone(), record, scenario));
            continue;
        }
        let attempt = Attempt {
            run: run.clone(),
            started: now,
            bytes: 0,
            observation: crate::assertions::Observation::default(),
            outcome: Outcome::Cancelled,
            elapsed: None,
        };
        workers.spawn(execute_arrival(worker, config.clone(), record, attempt));
    }
    let mut progress = run.lock();
    let end = progress
        .scheduling_end
        .unwrap_or_else(Instant::now)
        .min(deadline);
    let due = arrival_due(
        end.saturating_duration_since(started),
        rate,
        config.duration,
    );
    let remaining = due.saturating_sub(processed);
    progress.arrival.offered += remaining;
    progress.arrival.missed_deadlines += remaining;
    false
}

async fn execute_arrival(
    _worker: Worker,
    config: Arc<ValidatedConfig>,
    record: usize,
    mut attempt: Attempt,
) {
    attempt.started = Instant::now();
    let result = crate::transport::execute(
        &config,
        record_request(&config, record),
        &mut attempt.bytes,
        &mut attempt.observation,
    )
    .await;
    attempt.elapsed = Some(attempt.started.elapsed());
    attempt.outcome = match result {
        Ok(success) => Outcome::Response(success),
        Err(failure) => {
            attempt.run.diagnostics.record(failure);
            attempt.observation.cause = Some(failure.diagnostic().0);
            match failure {
                crate::transport::Failure::Timeout => Outcome::Timeout,
                crate::transport::Failure::Transport(_) => Outcome::Transport,
            }
        }
    };
}

fn select_record(run: &Run, progress: &mut Progress, cached: &mut Option<usize>) -> Option<usize> {
    let Some(summary) = &mut progress.dataset else {
        return Some(usize::MAX);
    };
    if summary.lifetime == crate::DataLifetime::Run
        && let Some(index) = progress.run_record
    {
        return Some(index);
    }
    if summary.lifetime == crate::DataLifetime::User
        && let Some(index) = *cached
    {
        return Some(index);
    }
    let index = progress.allocator.as_mut()?.take(summary);
    if let Some(index) = index {
        match summary.lifetime {
            crate::DataLifetime::Run => progress.run_record = Some(index),
            crate::DataLifetime::User => *cached = Some(index),
            crate::DataLifetime::Iteration => {}
        }
    } else {
        if summary.exhaustion == crate::DataExhaustion::Fail {
            progress.failure_code = Some("DATASET_EXHAUSTED".into());
        }
        progress.state = RunState::Stopping;
        progress.scheduling_end.get_or_insert_with(Instant::now);
        run.stop.cancel();
    }
    index
}

fn record_request(
    config: &ValidatedConfig,
    index: usize,
) -> Option<&crate::config::PreparedRequest> {
    if index == usize::MAX {
        None
    } else {
        Some(&config.dataset_requests[index])
    }
}

struct WorkerControl {
    id: tokio::task::Id,
    retire: CancellationToken,
    finished: CancellationToken,
}

struct Worker {
    run: Arc<Run>,
    retire: CancellationToken,
    finished: CancellationToken,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let mut progress = self.run.lock();
        progress.active_vus -= 1;
        if self.retire.is_cancelled() {
            progress.retiring_vus -= 1;
        }
        self.finished.cancel();
    }
}

struct Attempt {
    run: Arc<Run>,
    started: Instant,
    bytes: u64,
    observation: crate::assertions::Observation,
    outcome: Outcome,
    elapsed: Option<Duration>,
}

impl Drop for Attempt {
    fn drop(&mut self) {
        let outcome = std::mem::replace(&mut self.outcome, Outcome::Cancelled);
        let elapsed = self.elapsed.unwrap_or_else(|| self.started.elapsed());
        if matches!(outcome, Outcome::Cancelled) {
            self.observation.cause = Some("cancelled");
        }
        let mut progress = self.run.measured_lock();
        progress.metrics.inspect(&self.observation, outcome);
        progress.metrics.observe(outcome, elapsed, self.bytes);
    }
}

async fn execute_worker(worker: Worker, config: Arc<ValidatedConfig>, deadline: Instant) {
    if config.scenarios.is_some() {
        scenarios::worker(worker, config, deadline).await;
        return;
    }
    let mut cached = None;
    loop {
        let record;
        {
            let mut progress = worker.run.measured_lock();
            if progress.state != RunState::Running
                || worker.retire.is_cancelled()
                || Instant::now() >= deadline
            {
                break;
            }
            let Some(index) = select_record(&worker.run, &mut progress, &mut cached) else {
                break;
            };
            record = index;
            progress.metrics.admission();
        }
        let mut attempt = Attempt {
            run: worker.run.clone(),
            started: Instant::now(),
            bytes: 0,
            observation: crate::assertions::Observation::default(),
            outcome: Outcome::Cancelled,
            elapsed: None,
        };
        let result = crate::transport::execute(
            &config,
            record_request(&config, record),
            &mut attempt.bytes,
            &mut attempt.observation,
        )
        .await;
        attempt.elapsed = Some(attempt.started.elapsed());
        attempt.outcome = match result {
            Ok(success) => Outcome::Response(success),
            Err(failure) => {
                attempt.run.diagnostics.record(failure);
                attempt.observation.cause = Some(failure.diagnostic().0);
                match failure {
                    crate::transport::Failure::Timeout => Outcome::Timeout,
                    crate::transport::Failure::Transport(_) => Outcome::Transport,
                }
            }
        };
        drop(attempt);
        if config.pacing.is_zero() {
            tokio::task::yield_now().await;
        } else {
            tokio::select! {
                biased;
                _ = worker.run.stop.cancelled() => break,
                _ = worker.retire.cancelled() => break,
                _ = tokio::time::sleep_until(deadline) => break,
                _ = tokio::time::sleep(config.pacing) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RampStage, Workload};

    #[tokio::test]
    #[should_panic(expected = "worker programming bug")]
    async fn worker_panics_are_propagated_instead_of_failed_run_errors() {
        let task = tokio::spawn(async { panic!("worker programming bug") });
        task_failed(task.await);
    }

    #[tokio::test]
    #[should_panic(expected = "run progress was poisoned by a programming panic")]
    async fn poisoned_progress_is_not_reused() {
        let manager = RunManager::default();
        manager
            .start(
                RunConfig {
                    url: "http://127.0.0.1:1/".into(),
                    grace_ms: 0,
                    ..RunConfig::default()
                },
                true,
            )
            .await
            .unwrap();
        manager.shutdown().await;
        let run = manager
            .state
            .lock()
            .await
            .current
            .as_ref()
            .unwrap()
            .run
            .clone();
        drop(manager);
        let poisoned = run.clone();
        assert!(
            std::thread::spawn(move || {
                let _guard = poisoned.lock();
                panic!("initial programming bug");
            })
            .join()
            .is_err()
        );
        drop(run.lock());
    }

    #[test]
    fn arrival_deadlines_keep_fractional_rates_and_duration_endpoint_exact() {
        let duration = Duration::from_secs(1);
        assert_eq!(arrival_due(Duration::ZERO, 3, duration), 1);
        assert_eq!(
            arrival_due(Duration::from_nanos(333_333_333), 3, duration),
            1
        );
        assert_eq!(
            arrival_due(Duration::from_nanos(333_333_334), 3, duration),
            2
        );
        assert_eq!(arrival_offset(1, 3), Duration::from_nanos(333_333_334));
        assert_eq!(arrival_due(duration, 3, duration), 3);
        assert_eq!(
            arrival_due(Duration::from_millis(999), 1_000, duration),
            1_000
        );
        assert_eq!(
            arrival_due(duration, u32::MAX, duration),
            u64::from(u32::MAX)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn arrival_delayed_scheduler_skips_slots_without_replay() {
        let manager = RunManager::default();
        let input = RunConfig {
            url: "http://127.0.0.1:1/".into(),
            workload: Workload::Arrival {
                rate: 100,
                capacity: 4,
                duration_ms: 1_000,
            },
            grace_ms: 0,
            ..RunConfig::default()
        };
        manager.start(input, true).await.unwrap();
        tokio::task::yield_now().await;
        let before = manager.current().await.unwrap();
        tokio::time::advance(Duration::from_millis(500)).await;
        tokio::task::yield_now().await;
        let delayed = manager.current().await.unwrap();
        assert!(delayed.metrics.counters.started <= before.metrics.counters.started + 1);
        assert!(delayed.arrival.missed_deadlines >= 49);
        assert!(delayed.arrival.max_lateness_us >= 490_000);
        manager.shutdown().await;
        let summary = manager.current().await.unwrap();
        assert_eq!(
            summary.arrival.offered,
            summary.metrics.counters.started
                + summary.arrival.capacity_drops
                + summary.arrival.missed_deadlines
        );
        assert_eq!(summary.active_vus, 0);
        assert_eq!(summary.metrics.counters.in_flight, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn zero_worker_stages_keep_scheduling_until_completion() {
        let manager = RunManager::default();
        let config = RunConfig {
            url: "http://127.0.0.1:1/".into(),
            workload: Workload::Ramping {
                start_vus: 0,
                stages: vec![
                    RampStage {
                        duration_ms: 1_000,
                        target_vus: 0,
                    },
                    RampStage {
                        duration_ms: 2_000,
                        target_vus: 0,
                    },
                ],
            },
            ..RunConfig::default()
        };
        manager.start(config, true).await.unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        let current = manager.current().await.unwrap();
        assert_eq!(current.state, RunState::Running);
        assert_eq!(current.active_vus, 0);
        assert_eq!(current.metrics.counters.started, 0);
        tokio::time::advance(Duration::from_secs(2)).await;
        tokio::task::yield_now().await;
        let summary = manager.current().await.unwrap();
        assert_eq!(summary.state, RunState::Completed);
        assert_eq!(summary.scheduling_us, 3_000_000);
        assert_eq!(summary.active_vus, 0);
        manager.shutdown().await;
    }

    #[tokio::test(start_paused = true)]
    async fn stop_during_initial_zero_stage_cancels_future_ramp() {
        let manager = RunManager::default();
        let config = RunConfig {
            url: "http://127.0.0.1:1/".into(),
            workload: Workload::Ramping {
                start_vus: 0,
                stages: vec![
                    RampStage {
                        duration_ms: 1_000,
                        target_vus: 0,
                    },
                    RampStage {
                        duration_ms: 1_000,
                        target_vus: 4,
                    },
                ],
            },
            ..RunConfig::default()
        };
        let started = manager.start(config, true).await.unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(500)).await;
        manager.stop(&started.run_id).await.unwrap();
        manager.shutdown().await;
        let summary = manager.current().await.unwrap();
        assert_eq!(summary.state, RunState::Cancelled);
        assert_eq!(summary.scheduling_us, 500_000);
        assert_eq!(summary.desired_vus, 0);
        assert_eq!(summary.active_vus, 0);
        assert_eq!(summary.retiring_vus, 0);
        assert_eq!(summary.metrics.counters.started, 0);
    }
}

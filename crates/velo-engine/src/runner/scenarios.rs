//! Scenario admission and lifecycle execution under the existing worker budget.

use super::*;
use crate::{
    PhaseOutcome, ScenarioPhase, ScenarioStep, StepFailure,
    scenario_runtime::{Exchange, PreparedPlan, Session},
};

#[derive(Clone, Copy)]
enum Population {
    Setup,
    Warmup,
    Evaluated,
    Cleanup,
}

impl Population {
    fn phase(self) -> ScenarioPhase {
        match self {
            Self::Setup => ScenarioPhase::Setup,
            Self::Warmup | Self::Evaluated => ScenarioPhase::Load,
            Self::Cleanup => ScenarioPhase::Cleanup,
        }
    }
}

const PHASE_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn admit(progress: &mut Progress, plan: Option<&PreparedPlan>) -> Option<usize> {
    let metrics = progress.scenarios.as_mut()?;
    let plan = plan.expect("scenario metrics have a validated plan");
    let index = crate::scenario_runtime::choose(&plan.definition, metrics.admitted);
    metrics.admitted = metrics
        .admitted
        .checked_add(1)
        .expect("scenario admission counter exhausted");
    metrics.iterations[index].started += 1;
    Some(index)
}

fn selected_plan(config: &ValidatedConfig, record: usize) -> &PreparedPlan {
    record_request(config, record)
        .and_then(|record| record.scenarios.as_ref())
        .or(config.scenarios.as_ref())
        .expect("scenario worker has a validated plan")
}

fn new_session(run: &Run) -> Session {
    run.setup_session
        .lock()
        .expect("setup session poisoned by a programming panic")
        .clone()
}

struct Iteration {
    run: Arc<Run>,
    index: usize,
    result: Option<bool>,
    evaluated: bool,
}

impl Drop for Iteration {
    fn drop(&mut self) {
        let mut progress = self.run.measured_lock();
        let Some(metrics) = progress.scenarios.as_mut() else {
            return;
        };
        let totals = &mut metrics.iterations[self.index];
        if let Some(failed) = self.result {
            totals.completed += 1;
            totals.failed += u64::from(failed);
            progress.metrics.iteration(failed);
            if self.evaluated {
                progress.evaluated.iteration(failed);
            }
        } else {
            totals.cancelled += 1;
        }
    }
}

pub(super) async fn arrival(
    worker: Worker,
    config: Arc<ValidatedConfig>,
    record: usize,
    scenario: usize,
    evaluated: bool,
) {
    let mut session = new_session(&worker.run);
    iteration(
        &worker.run,
        &config,
        selected_plan(&config, record),
        &mut session,
        scenario,
        evaluated,
    )
    .await;
}

pub(super) async fn worker(worker: Worker, config: Arc<ValidatedConfig>, deadline: Instant) {
    let mut session = new_session(&worker.run);
    let mut cached = None;
    loop {
        let (record, scenario, evaluated) = {
            let mut progress = worker.run.measured_lock();
            if progress.state != RunState::Running
                || worker.retire.is_cancelled()
                || Instant::now() >= deadline
            {
                break;
            }
            let Some(record) = select_record(&worker.run, &mut progress, &mut cached) else {
                break;
            };
            let scenario = admit(&mut progress, config.scenarios.as_ref())
                .expect("scenario worker has metrics");
            (
                record,
                scenario,
                worker.run.evaluated(&progress, Instant::now()),
            )
        };
        iteration(
            &worker.run,
            &config,
            selected_plan(&config, record),
            &mut session,
            scenario,
            evaluated,
        )
        .await;
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

async fn iteration(
    run: &Arc<Run>,
    config: &ValidatedConfig,
    plan: &PreparedPlan,
    session: &mut Session,
    index: usize,
    evaluated: bool,
) {
    let mut guard = Iteration {
        run: run.clone(),
        index,
        result: None,
        evaluated,
    };
    let scenario = &plan.definition.scenarios[index];
    let result = steps(
        run,
        config,
        plan,
        session,
        &scenario.steps,
        if evaluated {
            Population::Evaluated
        } else {
            Population::Warmup
        },
        &scenario.id,
    )
    .await;
    guard.result = result;
}

pub(super) async fn phase(
    run: &Arc<Run>,
    config: &ValidatedConfig,
    plan: &PreparedPlan,
    session: &mut Session,
    phase: ScenarioPhase,
) -> PhaseOutcome {
    let definitions = if phase == ScenarioPhase::Setup {
        &plan.definition.setup
    } else {
        &plan.definition.cleanup
    };
    let outcome = if definitions.is_empty() {
        PhaseOutcome::Skipped
    } else {
        PhaseOutcome::Running
    };
    set_phase(run, phase, outcome);
    if definitions.is_empty() {
        return outcome;
    }
    let population = if phase == ScenarioPhase::Setup {
        Population::Setup
    } else {
        Population::Cleanup
    };
    let execution = steps(run, config, plan, session, definitions, population, "");
    let outcome = match tokio::time::timeout(PHASE_TIMEOUT, execution).await {
        Ok(Some(false)) => PhaseOutcome::Succeeded,
        Ok(Some(true)) | Err(_) => PhaseOutcome::Failed,
        Ok(None) => PhaseOutcome::Cancelled,
    };
    set_phase(run, phase, outcome);
    outcome
}

fn set_phase(run: &Run, phase: ScenarioPhase, outcome: PhaseOutcome) {
    if let Some(metrics) = run.lock().scenarios.as_mut() {
        if phase == ScenarioPhase::Setup {
            metrics.setup = outcome;
        } else {
            metrics.cleanup = outcome;
        }
    }
}

async fn steps(
    run: &Arc<Run>,
    config: &ValidatedConfig,
    plan: &PreparedPlan,
    session: &mut Session,
    definitions: &[ScenarioStep],
    population: Population,
    scenario_id: &str,
) -> Option<bool> {
    let phase = population.phase();
    let mut failed = false;
    for definition in definitions {
        if phase != ScenarioPhase::Cleanup && run.stop.is_cancelled() {
            return None;
        }
        let index = run
            .lock()
            .scenarios
            .as_ref()
            .expect("scenario report exists")
            .steps
            .iter()
            .position(|step| {
                step.totals.phase == phase
                    && step.totals.scenario_id == scenario_id
                    && step.totals.step_id == definition.id
            })
            .expect("validated step has metrics");
        let success = if phase == ScenarioPhase::Setup {
            tokio::select! {
                biased;
                _ = run.stop.cancelled() => return None,
                result = step(run, config, plan, session, definition, population, index) => result,
            }
        } else {
            step(run, config, plan, session, definition, population, index).await
        };
        failed |= !success;
        if !success && definition.on_failure == StepFailure::Stop {
            break;
        }
        let delay = crate::scenario_runtime::think_time(definition);
        if !delay.is_zero() {
            if phase == ScenarioPhase::Cleanup {
                tokio::time::sleep(delay).await;
            } else {
                tokio::select! {
                    biased;
                    _ = run.stop.cancelled() => return None,
                    _ = tokio::time::sleep(delay) => {}
                }
            }
        }
    }
    Some(failed)
}

struct StepAttempt {
    run: Arc<Run>,
    index: usize,
    population: Population,
    started: Instant,
    bytes: u64,
    observation: crate::assertions::Observation,
    outcome: Outcome,
    error: Option<&'static str>,
    success: bool,
}

impl Drop for StepAttempt {
    fn drop(&mut self) {
        let elapsed = self.started.elapsed();
        let mut progress = self.run.measured_lock();
        let metrics = progress.scenarios.as_mut().expect("scenario report exists");
        let metric = &mut metrics.steps[self.index];
        if matches!(self.outcome, Outcome::Cancelled) {
            metric.totals.cancelled += 1;
        } else if !self.success {
            metric.totals.failed += 1;
        }
        if matches!(self.outcome, Outcome::Response(_)) {
            metric.latency.record(elapsed, &mut false);
        }
        if let Some(error) = self.error {
            metric.totals.last_error = Some(error.into());
            if error.starts_with("EXTRACTION_") || error.starts_with("SESSION_") {
                metric.totals.extraction_failures += 1;
            }
        }
        if self.population.phase() == ScenarioPhase::Load {
            progress
                .metrics
                .scenario_checks(&self.observation, self.outcome);
            progress.metrics.observe(self.outcome, elapsed, self.bytes);
            if matches!(self.population, Population::Evaluated) {
                progress
                    .evaluated
                    .scenario_checks(&self.observation, self.outcome);
                progress
                    .evaluated
                    .observe(self.outcome, elapsed, self.bytes);
                let now = Instant::now();
                let started = progress.started.expect("load step has start");
                let boundary =
                    started + Duration::from_millis(self.run.confirmation.warmup_ms.into());
                progress.live.observe(
                    now.saturating_duration_since(boundary).as_micros() as u64,
                    self.outcome,
                    elapsed,
                    &self.observation,
                );
            }
        }
    }
}

async fn step(
    run: &Arc<Run>,
    config: &ValidatedConfig,
    plan: &PreparedPlan,
    session: &mut Session,
    definition: &ScenarioStep,
    population: Population,
    index: usize,
) -> bool {
    let phase = population.phase();
    let allowance =
        plan.base.budget.max_request_bytes as usize / (config.confirmation.vus as usize + 1);
    let prepared = crate::scenario_runtime::check_binding_budget(
        &definition.request,
        &session.values,
        allowance,
    )
    .and_then(|()| crate::authoring::bind_scenario_request(&definition.request, &session.values))
    .map(|request| crate::scenario_runtime::request_config(&plan.base, request))
    .and_then(|request| {
        crate::config::validate_request_parts(
            request,
            Some(config.client.clone()),
            plan.multipart[index].as_deref(),
        )
    })
    .and_then(|prepared| {
        if prepared.confirmation.origin != config.confirmation.origin {
            Err(crate::scenario_runtime::invalid(
                "Resolved requests must retain the authorized origin.",
            ))
        } else {
            Ok(prepared)
        }
    });
    let mut prepared = match prepared {
        Ok(prepared) => prepared,
        Err(_) => {
            session.invalidate(&definition.extractions);
            let mut progress = run.measured_lock();
            let totals = &mut progress
                .scenarios
                .as_mut()
                .expect("scenario report exists")
                .steps[index]
                .totals;
            totals.binding_failures += 1;
            totals.failed += 1;
            totals.last_error = Some("SCENARIO_BINDING_FAILED".into());
            return false;
        }
    };
    prepared.uploads = config.uploads.clone();
    prepared.upload_stop = config.upload_stop.child_token();
    {
        let mut progress = run.measured_lock();
        progress
            .scenarios
            .as_mut()
            .expect("scenario report exists")
            .steps[index]
            .totals
            .requests += 1;
        if phase == ScenarioPhase::Load {
            progress.metrics.admission();
            if matches!(population, Population::Evaluated) {
                progress.evaluated.admission();
            }
        }
    }
    let mut attempt = StepAttempt {
        run: run.clone(),
        index,
        population,
        started: Instant::now(),
        bytes: 0,
        observation: crate::assertions::Observation::default(),
        outcome: Outcome::Cancelled,
        error: None,
        success: false,
    };
    let mut exchange = Exchange {
        session,
        step: definition,
        error: None,
    };
    let result = crate::transport::execute_scenario(
        &prepared,
        &mut attempt.bytes,
        &mut attempt.observation,
        &mut exchange,
    )
    .await;
    attempt.error = exchange.error;
    attempt.outcome = match result {
        Ok(success) => Outcome::Response(success),
        Err(failure) => {
            run.diagnostics.record(failure);
            attempt.error.get_or_insert("SCENARIO_REQUEST_FAILED");
            exchange.session.invalidate(&definition.extractions);
            match failure {
                crate::transport::Failure::Timeout => Outcome::Timeout,
                _ => Outcome::Transport,
            }
        }
    };
    let assertions_passed = attempt
        .observation
        .checks
        .iter()
        .all(|outcome| *outcome == crate::CheckOutcome::Passed);
    if !assertions_passed {
        attempt.error.get_or_insert("SCENARIO_ASSERTION_FAILED");
    }
    if matches!(attempt.outcome, Outcome::Response(false)) {
        attempt.error.get_or_insert("SCENARIO_HTTP_FAILED");
    }
    attempt.success = matches!(attempt.outcome, Outcome::Response(true))
        && assertions_passed
        && attempt.error.is_none();
    if !attempt.success {
        exchange.session.invalidate(&definition.extractions);
    }
    attempt.success
}

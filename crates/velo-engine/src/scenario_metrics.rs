//! Fixed-cardinality journey accounting; response values never enter reports.

use serde::Serialize;
use ts_rs::TS;

use crate::metrics::{Distribution, LatencySnapshot};

/// Lifecycle phase outcome, published only after its owned work has settled.
///
/// # Examples
/// ```
/// let _outcome = velo_engine::PhaseOutcome::Pending;
/// ```
#[derive(Clone, Copy, Debug, Serialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PhaseOutcome {
    /// Not started yet.
    Pending,
    /// Executing ordered actions.
    Running,
    /// Every configured action succeeded.
    Succeeded,
    /// At least one action failed.
    Failed,
    /// Interrupted before all actions completed.
    Cancelled,
    /// No actions configured.
    Skipped,
}

/// Measurement population for a configured step identity.
///
/// # Examples
/// ```
/// let _phase = velo_engine::ScenarioPhase::Load;
/// ```
#[derive(Clone, Copy, Debug, Serialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioPhase {
    /// Once before measured load.
    Setup,
    /// Admitted scenario iterations.
    Load,
    /// Once after owned load tasks settle.
    Cleanup,
}

/// Iteration counts for one configured scenario.
///
/// # Examples
/// ```
/// # fn example(report: velo_engine::ScenarioTotals) {
/// let _report = report;
/// # }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct ScenarioTotals {
    /// Stable configured scenario identity.
    pub id: String,
    /// Admitted iterations.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub started: u64,
    /// Non-cancelled iterations, including failures.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub completed: u64,
    /// Completed iterations with any step failure.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub failed: u64,
    /// Iterations interrupted before completion.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub cancelled: u64,
}

/// Bounded request and binding outcomes for one configured step.
///
/// # Examples
/// ```
/// # fn example(report: velo_engine::StepTotals) {
/// let _report = report;
/// # }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct StepTotals {
    /// Configured scenario identity; empty for setup and cleanup.
    pub scenario_id: String,
    /// Configured step identity.
    pub step_id: String,
    /// Measurement population.
    pub phase: ScenarioPhase,
    /// HTTP attempts started; excludes local binding failures.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub requests: u64,
    /// Step failures, including binding and extraction failures.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub failed: u64,
    /// HTTP attempts interrupted during drain.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub cancelled: u64,
    /// Requests rejected locally before sending.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub binding_failures: u64,
    /// Responses with failed required extraction or state limits.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub extraction_failures: u64,
    /// Latest stable failure code; never response content.
    pub last_error: Option<String>,
    /// Complete HTTP response latency, excluding think time.
    pub response_latency: LatencySnapshot,
}

/// Safe journey totals retained in snapshots and durable history.
///
/// # Examples
/// ```
/// # fn example(report: velo_engine::ScenarioReport) {
/// let _report = report;
/// # }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct ScenarioReport {
    /// Bounded configured iteration populations.
    #[ts(type = "ScenarioTotals[]")]
    pub iterations: Vec<ScenarioTotals>,
    /// At most 64 fixed step populations; dynamic URLs never create identities.
    #[ts(type = "StepTotals[]")]
    pub steps: Vec<StepTotals>,
    /// Setup outcome; failures prevent load.
    pub setup: PhaseOutcome,
    /// Cleanup outcome; failure never implies successful removal.
    pub cleanup: PhaseOutcome,
}

pub(crate) struct StepMetrics {
    pub totals: StepTotals,
    pub latency: Distribution,
}

pub(crate) struct ScenarioMetrics {
    pub iterations: Vec<ScenarioTotals>,
    pub steps: Vec<StepMetrics>,
    pub setup: PhaseOutcome,
    pub cleanup: PhaseOutcome,
    pub admitted: u64,
}

impl ScenarioMetrics {
    pub(crate) fn new(plan: &crate::ScenarioPlan) -> Self {
        let capacity = plan.setup.len()
            + plan.cleanup.len()
            + plan
                .scenarios
                .iter()
                .map(|scenario| scenario.steps.len())
                .sum::<usize>();
        let mut steps = Vec::with_capacity(capacity);
        for (phase, id, definitions) in std::iter::once((ScenarioPhase::Setup, "", &plan.setup))
            .chain(
                plan.scenarios
                    .iter()
                    .map(|scenario| (ScenarioPhase::Load, scenario.id.as_str(), &scenario.steps)),
            )
            .chain(std::iter::once((ScenarioPhase::Cleanup, "", &plan.cleanup)))
        {
            for step in definitions {
                steps.push(StepMetrics {
                    totals: StepTotals {
                        scenario_id: id.into(),
                        step_id: step.id.clone(),
                        phase,
                        requests: 0,
                        failed: 0,
                        cancelled: 0,
                        binding_failures: 0,
                        extraction_failures: 0,
                        last_error: None,
                        response_latency: Distribution::new().snapshot(),
                    },
                    latency: Distribution::new(),
                });
            }
        }
        Self {
            iterations: plan
                .scenarios
                .iter()
                .map(|scenario| ScenarioTotals {
                    id: scenario.id.clone(),
                    started: 0,
                    completed: 0,
                    failed: 0,
                    cancelled: 0,
                })
                .collect(),
            steps,
            setup: PhaseOutcome::Pending,
            cleanup: PhaseOutcome::Pending,
            admitted: 0,
        }
    }

    pub(crate) fn snapshot(&self) -> ScenarioReport {
        ScenarioReport {
            iterations: self.iterations.clone(),
            steps: self
                .steps
                .iter()
                .map(|step| {
                    let mut totals = step.totals.clone();
                    totals.response_latency = step.latency.snapshot();
                    totals
                })
                .collect(),
            setup: self.setup,
            cleanup: self.cleanup,
        }
    }
}

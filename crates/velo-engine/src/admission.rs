//! Monotonic measurement boundaries and bounded live admission policies.
//!
//! Scheduling duration includes warm-up. Requests belong to the population selected
//! at admission; a late response never changes that membership. Evaluated duration
//! extends from the warm-up boundary through load drain, excluding setup and cleanup.
//! Live policies sample completed evaluated requests in closed 100 ms buckets.
//! Windows and sustained durations use those monotonic boundaries, never wall time.
//! Empty, unavailable, or corrupt observations reset sustained breach evidence.
//! A missed observation tick also resets the sustained timer; unobserved time is
//! never inferred to be breached. Storage is bounded to 602 shared buckets and
//! eight policy accumulators, independent of request count. Expired latency buffers
//! are reused, and contiguous checks add and subtract only entering/leaving buckets.

use std::{collections::VecDeque, time::Duration};

use hdrhistogram::Histogram;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{RunError, Threshold, ThresholdMetric, ThresholdOperator, metrics::Outcome};

const TICK_US: u64 = 100_000;
const MAX_WINDOW_MS: u32 = 60_000;
const MAX_POLICIES: usize = 8;

/// A live acceptable metric rule with a complete observation window and sustained breach.
///
/// Latency uses a two-significant-digit HDR p95 in milliseconds. Failure rates use
/// terminal requests or assertion evaluations. Request rate uses requests/second,
/// including scenario step requests, rather than scenario iterations/second.
///
/// # Examples
/// ```
/// use velo_engine::{AbortPolicy, Threshold, ThresholdMetric, ThresholdOperator};
/// let policy = AbortPolicy {
///     rule: Threshold { metric: ThresholdMetric::HttpFailurePercent,
///         operator: ThresholdOperator::LessThan, limit: 20.0 },
///     window_ms: 1_000, min_samples: 10, sustained_ms: 2_000,
/// };
/// assert_eq!(policy.min_samples, 10);
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AbortPolicy {
    /// Acceptable strict comparison; failure can close admission after sustained evidence.
    pub rule: Threshold,
    /// Complete trailing window, 100–60,000 ms in multiples of 100 ms.
    pub window_ms: u32,
    /// Required terminal requests, response latencies, or assertion evaluations.
    pub min_samples: u32,
    /// Consecutive breached observation duration, 100–60,000 ms in 100 ms steps.
    pub sustained_ms: u32,
}

/// Triggering live policy evidence retained with partial results and history.
///
/// # Examples
/// ```
/// fn trigger_seconds(evidence: &velo_engine::AbortEvidence) -> f64 {
///     evidence.at_us as f64 / 1_000_000.0
/// }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct AbortEvidence {
    /// Policy that closed admission; the first configured simultaneous breach wins.
    pub policy: AbortPolicy,
    /// Last unrounded observation in the configured metric's unit.
    pub observed: Option<f64>,
    /// Metric-specific population in the triggering complete window.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub samples: u64,
    /// Monotonic time from load start when admission closed, including warm-up.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub at_us: u64,
    /// Continuous observed breach duration, excluding initial window coverage.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub breached_us: u64,
}

/// Evaluated population timing, independent of full-run request accounting.
///
/// Requests are assigned at admission. Responses and cancellations retain their
/// membership through drain. Final thresholds use this population even when partial.
///
/// # Examples
/// ```
/// fn partial(window: &velo_engine::MeasurementWindow) -> bool { !window.complete }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct MeasurementWindow {
    /// Scheduling offset where evaluated admissions begin; zero disables warm-up.
    pub warmup_ms: u32,
    /// Observed duration after warm-up through drain; zero before the boundary.
    #[serde(serialize_with = "crate::metrics::decimal")]
    #[ts(type = "string")]
    pub evaluated_us: u64,
    /// True only after all configured scheduling time and load cleanup completed.
    pub complete: bool,
}

pub(crate) fn validate(
    warmup_ms: u32,
    duration_ms: u32,
    policies: &[AbortPolicy],
) -> Result<(), RunError> {
    if warmup_ms >= duration_ms {
        return Err(RunError::new(
            "INVALID_MEASUREMENT_WINDOW",
            "Warm-up must be shorter than scheduling duration.",
            Some("warmup_ms"),
        ));
    }
    if policies.len() > MAX_POLICIES {
        return Err(invalid());
    }
    for policy in policies {
        crate::threshold::validate(std::slice::from_ref(&policy.rule))?;
        if policy.min_samples == 0
            || !(100..=MAX_WINDOW_MS).contains(&policy.window_ms)
            || !(100..=MAX_WINDOW_MS).contains(&policy.sustained_ms)
            || policy.window_ms % 100 != 0
            || policy.sustained_ms % 100 != 0
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn invalid() -> RunError {
    RunError::new(
        "INVALID_ABORT_POLICY",
        "Use at most eight policies, positive minimum samples, and window/sustained durations of 100–60,000 ms in 100 ms steps.",
        Some("abort_policies"),
    )
}

#[derive(Default)]
struct Bucket {
    index: u64,
    terminal: u64,
    failures: u64,
    checks: u64,
    check_failures: u64,
    unavailable: bool,
    overflow: bool,
    latency: Option<Histogram<u64>>,
}

fn increment(value: &mut u64, amount: u64, overflow: &mut bool) {
    if let Some(next) = value.checked_add(amount) {
        *value = next;
    } else {
        *overflow = true;
    }
}

impl Bucket {
    fn observe(
        &mut self,
        outcome: Outcome,
        elapsed: Duration,
        observation: &crate::assertions::Observation,
        latency: bool,
    ) {
        if matches!(outcome, Outcome::Cancelled) {
            return;
        }
        increment(&mut self.terminal, 1, &mut self.overflow);
        if !matches!(outcome, Outcome::Response(true)) {
            increment(&mut self.failures, 1, &mut self.overflow);
        }
        for check in &observation.checks {
            increment(&mut self.checks, 1, &mut self.overflow);
            match check {
                crate::CheckOutcome::Passed => {}
                crate::CheckOutcome::StatusMismatch
                | crate::CheckOutcome::MissingHeader
                | crate::CheckOutcome::HeaderMismatch
                | crate::CheckOutcome::MissingField
                | crate::CheckOutcome::JsonMismatch
                | crate::CheckOutcome::TextMismatch => {
                    increment(&mut self.check_failures, 1, &mut self.overflow);
                }
                _ => self.unavailable = true,
            }
        }
        self.unavailable |= !matches!(outcome, Outcome::Response(_));
        if latency && matches!(outcome, Outcome::Response(_)) {
            let histogram = self.latency.get_or_insert_with(histogram);
            let micros = elapsed.as_micros();
            if micros == 0
                || micros > crate::metrics::MAX_LATENCY_US.into()
                || histogram.record(micros as u64).is_err()
            {
                self.overflow = true;
            }
        }
    }
}

fn histogram() -> Histogram<u64> {
    Histogram::new_with_bounds(1, crate::metrics::MAX_LATENCY_US, 2)
        .expect("fixed live histogram bounds are valid")
}

pub(crate) struct LivePolicies {
    policies: Vec<AbortPolicy>,
    breached: Vec<Option<u64>>,
    buckets: VecDeque<Bucket>,
    last_tick: u64,
    max_ticks: u64,
    latency: bool,
    latencies: Vec<Option<Histogram<u64>>>,
    recycled: Option<Histogram<u64>>,
}

impl LivePolicies {
    pub(crate) fn new(policies: Vec<AbortPolicy>) -> Self {
        let max_ticks = policies
            .iter()
            .map(|policy| u64::from(policy.window_ms) / 100)
            .max()
            .unwrap_or(0);
        Self {
            breached: vec![None; policies.len()],
            latency: policies
                .iter()
                .any(|policy| matches!(policy.rule.metric, ThresholdMetric::ResponseP95Ms)),
            latencies: policies
                .iter()
                .map(|policy| {
                    matches!(policy.rule.metric, ThresholdMetric::ResponseP95Ms).then(histogram)
                })
                .collect(),
            policies,
            buckets: VecDeque::with_capacity(max_ticks as usize + 2),
            last_tick: 0,
            max_ticks,
            recycled: None,
        }
    }

    pub(crate) fn observe(
        &mut self,
        elapsed_us: u64,
        outcome: Outcome,
        elapsed: Duration,
        observation: &crate::assertions::Observation,
    ) {
        if self.policies.is_empty() {
            return;
        }
        let index = elapsed_us / TICK_US;
        self.prune(index);
        if self
            .buckets
            .back()
            .is_none_or(|bucket| bucket.index != index)
        {
            self.buckets.push_back(Bucket {
                index,
                latency: self.recycled.take(),
                ..Bucket::default()
            });
        }
        if let Some(bucket) = self.buckets.back_mut() {
            bucket.observe(outcome, elapsed, observation, self.latency);
        }
    }

    fn prune(&mut self, tick: u64) {
        while self
            .buckets
            .front()
            .is_some_and(|bucket| bucket.index < tick.saturating_sub(self.max_ticks + 1))
        {
            if let Some(bucket) = self.buckets.pop_front()
                && let Some(mut histogram) = bucket.latency
            {
                histogram.reset();
                self.recycled = Some(histogram);
            }
        }
    }

    pub(crate) fn evaluate(&mut self, elapsed_us: u64) -> Option<AbortEvidence> {
        let tick = elapsed_us / TICK_US;
        if tick <= self.last_tick || self.policies.is_empty() {
            return None;
        }
        let contiguous = tick == self.last_tick + 1;
        self.last_tick = tick;
        self.prune(tick);
        for (index, policy) in self.policies.iter().enumerate() {
            let ticks = u64::from(policy.window_ms) / 100;
            if tick < ticks {
                continue;
            }
            let mut terminal = 0;
            let mut failures = 0;
            let mut checks = 0;
            let mut check_failures = 0;
            let mut unavailable = false;
            let mut overflow = false;
            let mut latency = self.latencies[index].as_mut();
            if let Some(histogram) = &mut latency {
                if contiguous && tick > ticks {
                    for bucket in &self.buckets {
                        if let Some(values) = &bucket.latency {
                            if bucket.index == tick - 1 {
                                overflow |= histogram.add(values).is_err();
                            }
                            if bucket.index.checked_add(ticks + 1) == Some(tick) {
                                overflow |= histogram.subtract(values).is_err();
                            }
                        }
                    }
                } else {
                    histogram.reset();
                    for bucket in &self.buckets {
                        if bucket.index >= tick - ticks
                            && bucket.index < tick
                            && let Some(values) = &bucket.latency
                        {
                            overflow |= histogram.add(values).is_err();
                        }
                    }
                }
            }
            for bucket in &self.buckets {
                if bucket.index < tick - ticks || bucket.index >= tick {
                    continue;
                }
                increment(&mut terminal, bucket.terminal, &mut overflow);
                increment(&mut failures, bucket.failures, &mut overflow);
                increment(&mut checks, bucket.checks, &mut overflow);
                increment(&mut check_failures, bucket.check_failures, &mut overflow);
                unavailable |= bucket.unavailable;
                overflow |= bucket.overflow;
            }
            let (samples, observed) = match policy.rule.metric {
                ThresholdMetric::HttpFailurePercent => (
                    terminal,
                    (terminal > 0).then(|| failures as f64 * 100.0 / terminal as f64),
                ),
                ThresholdMetric::AssertionFailurePercent => (
                    checks,
                    (checks > 0 && !unavailable)
                        .then(|| check_failures as f64 * 100.0 / checks as f64),
                ),
                ThresholdMetric::RequestRate => (
                    terminal,
                    (terminal > 0).then(|| terminal as f64 * 1000.0 / f64::from(policy.window_ms)),
                ),
                ThresholdMetric::ResponseP95Ms => {
                    let histogram = latency.expect("latency policies own a histogram");
                    (
                        histogram.len(),
                        (!histogram.is_empty())
                            .then(|| histogram.value_at_quantile(0.95) as f64 / 1000.0),
                    )
                }
            };
            let observed = observed.filter(|value| {
                !overflow && value.is_finite() && samples >= u64::from(policy.min_samples)
            });
            let failed = observed.is_some_and(|value| match policy.rule.operator {
                ThresholdOperator::LessThan => value >= policy.rule.limit,
                ThresholdOperator::GreaterThan => value <= policy.rule.limit,
            });
            if !failed {
                self.breached[index] = None;
                continue;
            }
            if !contiguous {
                self.breached[index] = None;
            }
            let since = *self.breached[index].get_or_insert(tick * TICK_US);
            let breached_us = tick * TICK_US - since;
            if breached_us >= u64::from(policy.sustained_ms) * 1000 {
                return Some(AbortEvidence {
                    policy: policy.clone(),
                    observed,
                    samples,
                    at_us: elapsed_us,
                    breached_us,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policies() -> LivePolicies {
        LivePolicies::new(vec![AbortPolicy {
            rule: Threshold {
                metric: ThresholdMetric::HttpFailurePercent,
                operator: ThresholdOperator::LessThan,
                limit: 50.0,
            },
            window_ms: 200,
            min_samples: 2,
            sustained_ms: 200,
        }])
    }

    fn observe(live: &mut LivePolicies, millis: u64, success: bool) {
        live.observe(
            millis * 1000,
            Outcome::Response(success),
            Duration::from_millis(1),
            &crate::assertions::Observation::default(),
        );
    }

    #[test]
    fn full_window_minimum_samples_and_sustained_breach_are_required() {
        let mut live = policies();
        observe(&mut live, 1, false);
        assert!(live.evaluate(100_000).is_none());
        assert!(live.evaluate(200_000).is_none());
        observe(&mut live, 201, false);
        observe(&mut live, 202, false);
        assert!(live.evaluate(300_000).is_none());
        observe(&mut live, 301, false);
        assert!(live.evaluate(400_000).is_none());
        observe(&mut live, 401, false);
        let evidence = live.evaluate(500_000).unwrap();
        assert_eq!(evidence.breached_us, 200_000);
        assert_eq!(evidence.samples, 2);
        assert_eq!(evidence.observed, Some(100.0));
    }

    #[test]
    fn transient_breach_empty_windows_and_delayed_ticks_reset_evidence() {
        let mut live = policies();
        live.policies[0].rule.limit = 51.0;
        for tick in 0..3 {
            observe(&mut live, tick * 100 + 1, false);
            observe(&mut live, tick * 100 + 2, false);
            assert!(live.evaluate((tick + 1) * 100_000).is_none());
        }
        for tick in 3..7 {
            observe(&mut live, tick * 100 + 1, true);
            observe(&mut live, tick * 100 + 2, true);
            assert!(live.evaluate((tick + 1) * 100_000).is_none());
        }
        assert!(live.evaluate(900_000).is_none());
        for tick in [9, 10, 12, 13] {
            observe(&mut live, tick * 100 + 1, false);
            observe(&mut live, tick * 100 + 2, false);
            assert!(live.evaluate((tick + 1) * 100_000).is_none());
        }
        assert!(live.evaluate(1_600_000).is_none());
    }

    #[test]
    fn unavailable_assertions_and_out_of_range_latency_never_extend_breach() {
        for metric in [
            ThresholdMetric::AssertionFailurePercent,
            ThresholdMetric::ResponseP95Ms,
        ] {
            let mut live = LivePolicies::new(vec![AbortPolicy {
                rule: Threshold {
                    metric,
                    operator: ThresholdOperator::LessThan,
                    limit: 0.0,
                },
                window_ms: 100,
                min_samples: 1,
                sustained_ms: 100,
            }]);
            let mut observation = crate::assertions::Observation::default();
            observation.checks.push(crate::CheckOutcome::Unavailable);
            for tick in 0..5 {
                live.observe(
                    tick * 100_000 + 1,
                    Outcome::Response(false),
                    Duration::ZERO,
                    &observation,
                );
                assert!(live.evaluate((tick + 1) * 100_000).is_none());
            }
        }
    }

    #[test]
    fn rolling_latency_matches_rebuilt_windows_across_expiry_and_skipped_ticks() {
        let mut live = LivePolicies::new(vec![AbortPolicy {
            rule: Threshold {
                metric: ThresholdMetric::ResponseP95Ms,
                operator: ThresholdOperator::LessThan,
                limit: 3_600_000.0,
            },
            window_ms: 300,
            min_samples: 1,
            sustained_ms: 100,
        }]);
        for tick in 0..100 {
            for sample in 1..5 {
                live.observe(
                    tick * TICK_US + sample,
                    Outcome::Response(true),
                    Duration::from_millis((tick * sample) % 37 + 1),
                    &crate::assertions::Observation::default(),
                );
            }
            if tick % 7 == 5 {
                continue;
            }
            assert!(live.evaluate((tick + 1) * TICK_US).is_none());
            if tick < 2 {
                continue;
            }
            let mut expected = histogram();
            for previous in (tick + 1).saturating_sub(3)..=tick {
                for sample in 1..5 {
                    expected
                        .record(((previous * sample) % 37 + 1) * 1000)
                        .unwrap();
                }
            }
            let actual = live.latencies[0].as_ref().unwrap();
            assert_eq!(actual.len(), expected.len());
            assert_eq!(
                actual.value_at_quantile(0.95),
                expected.value_at_quantile(0.95)
            );
        }
    }

    #[test]
    fn policy_samples_use_the_metric_population_and_configuration_order() {
        let mut live = LivePolicies::new(vec![
            AbortPolicy {
                rule: Threshold {
                    metric: ThresholdMetric::AssertionFailurePercent,
                    operator: ThresholdOperator::LessThan,
                    limit: 50.0,
                },
                window_ms: 100,
                min_samples: 4,
                sustained_ms: 100,
            },
            AbortPolicy {
                rule: Threshold {
                    metric: ThresholdMetric::RequestRate,
                    operator: ThresholdOperator::GreaterThan,
                    limit: 30.0,
                },
                window_ms: 100,
                min_samples: 2,
                sustained_ms: 100,
            },
        ]);
        let observation = crate::assertions::Observation {
            checks: vec![crate::CheckOutcome::StatusMismatch; 2],
            ..Default::default()
        };
        for tick in 0..2 {
            for offset in 1..3 {
                live.observe(
                    tick * TICK_US + offset,
                    Outcome::Response(true),
                    Duration::from_millis(1),
                    &observation,
                );
            }
            let result = live.evaluate((tick + 1) * TICK_US);
            if tick == 0 {
                assert!(result.is_none());
            } else {
                let evidence = result.unwrap();
                assert_eq!(evidence.samples, 4);
                assert!(matches!(
                    evidence.policy.rule.metric,
                    ThresholdMetric::AssertionFailurePercent
                ));
                assert_eq!(evidence.observed, Some(100.0));
            }
        }
    }

    #[test]
    fn rolling_storage_is_bounded_independent_of_request_count() {
        let mut live = policies();
        for millis in 0..10_000 {
            observe(&mut live, millis, false);
            assert!(live.buckets.len() <= 4);
        }
    }
}

use std::time::Duration;

use hdrhistogram::Histogram;
use serde::{Serialize, Serializer};
use ts_rs::TS;

pub(crate) const MAX_LATENCY_US: u64 = 3_600_000_000;

pub(crate) fn decimal<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

/// Bounded HDR distribution, in microseconds, with three significant digits.
///
/// Empty populations have no statistics. Samples outside 1 μs–1 hour are counted
/// separately and excluded from percentiles. Percentiles and mean are approximate.
///
/// # Examples
/// Convert a response distribution to display units without inventing empty data:
/// ```
/// fn mean_milliseconds(latency: &velo_engine::LatencySnapshot) -> Option<f64> {
///     latency.mean_us.map(|micros| micros / 1_000.0)
/// }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct LatencySnapshot {
    /// Decimal significant digits used by the HDR histogram.
    pub significant_digits: u8,
    /// Lowest supported sample in microseconds.
    pub lowest_us: u32,
    /// Highest supported sample in microseconds.
    pub highest_us: u32,
    /// Number of recorded samples, encoded as a decimal string.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub samples: u64,
    /// Samples outside the supported range, encoded as a decimal string.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub out_of_range: u64,
    /// Approximate arithmetic mean in microseconds.
    pub mean_us: Option<f64>,
    /// Minimum recorded latency in microseconds.
    pub min_us: Option<u32>,
    /// Maximum recorded latency in microseconds.
    pub max_us: Option<u32>,
    /// Approximate median in microseconds.
    pub median_us: Option<u32>,
    /// Approximate 90th percentile in microseconds.
    pub p90_us: Option<u32>,
    /// Approximate 95th percentile in microseconds.
    pub p95_us: Option<u32>,
    /// Approximate 99th percentile in microseconds.
    pub p99_us: Option<u32>,
}

/// Cumulative logical-request accounting; all counters use decimal strings on IPC.
///
/// `started = responses + transport_failures + timeouts + cancelled + in_flight`.
/// A status failure is already included in responses. Cancellations are excluded
/// from the HTTP failure rate and terminal request throughput.
///
/// # Examples
/// ```
/// let counters = velo_engine::Counters::default();
/// assert_eq!(counters.started, counters.responses + counters.transport_failures
///     + counters.timeouts + counters.cancelled + counters.in_flight);
/// ```
#[derive(Clone, Debug, Default, Serialize, TS)]
pub struct Counters {
    /// Admitted logical requests.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub started: u64,
    /// Fully consumed HTTP responses, including status failures.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub responses: u64,
    /// Fully consumed responses with status 200–399.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub successes: u64,
    /// Fully consumed responses with other statuses.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub status_failures: u64,
    /// Failed connection, protocol, or response body operations, excluding timeouts.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub transport_failures: u64,
    /// Requests whose complete-response deadline expired.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub timeouts: u64,
    /// Requests cancelled after the drain grace expired.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub cancelled: u64,
    /// Requests still executing; zero in terminal states.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub in_flight: u64,
    /// Encoded response body bytes consumed, including bytes from failed attempts.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub response_body_bytes: u64,
    /// Completed non-cancelled iterations; scenarios may contain several requests.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub completed_iterations: u64,
    /// Iterations with HTTP failures; scenarios also include assertion, binding and extraction failures.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub failed_iterations: u64,
}

/// Cumulative bounded metrics. No individual request or target data is retained.
///
/// # Examples
/// An integrity warning should remain visible alongside the measured results:
/// ```
/// fn has_integrity_warning(metrics: &velo_engine::MetricsSnapshot) -> bool {
///     metrics.counter_overflow || metrics.aggregation_interference
///         || metrics.response_latency.out_of_range > 0
///         || metrics.transport_latency.out_of_range > 0
///         || metrics.timeout_latency.out_of_range > 0
/// }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct MetricsSnapshot {
    /// Assertion outcomes and bounded content-free diagnostic samples.
    pub diagnostics: crate::ResponseDiagnostics,
    /// Failed assertion evaluations over all evaluations; absent if any check is unavailable.
    pub assertion_failure_rate: Option<f64>,
    /// Reconciled request outcomes and received payload bytes.
    pub counters: Counters,
    /// Completed HTTP response latencies, including status failures.
    pub response_latency: LatencySnapshot,
    /// Transport failure durations, excluded from response latency.
    pub transport_latency: LatencySnapshot,
    /// Timeout durations, excluded from response latency.
    pub timeout_latency: LatencySnapshot,
    /// Failure fraction over responses, transport failures, and timeouts; None when empty.
    pub http_failure_rate: Option<f64>,
    /// Non-cancelled terminal requests per actual elapsed second.
    pub request_rate: f64,
    /// Completed non-cancelled iterations per actual second; independent of requests per iteration.
    pub iteration_rate: f64,
    /// Successful responses per actual elapsed second.
    pub success_rate: f64,
    /// Total time waiting to enter the metrics critical section, in microseconds.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub aggregation_wait_us: u64,
    /// At least one metrics lock acquisition waited over one millisecond.
    pub aggregation_interference: bool,
    /// At least one counter exhausted its representation; totals are then incomplete.
    pub counter_overflow: bool,
    /// Maximum concurrently in-flight requests.
    #[serde(serialize_with = "decimal")]
    #[ts(type = "string")]
    pub max_in_flight: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum Outcome {
    Response(bool),
    Transport,
    Timeout,
    Cancelled,
}

pub(crate) struct Distribution {
    histogram: Histogram<u64>,
    out_of_range: u64,
}

impl Distribution {
    pub(crate) fn new() -> Self {
        Self {
            histogram: Histogram::new_with_bounds(1, MAX_LATENCY_US, 3)
                .expect("fixed histogram bounds are valid"),
            out_of_range: 0,
        }
    }

    pub(crate) fn record(&mut self, elapsed: Duration, overflow: &mut bool) {
        let micros = elapsed.as_micros();
        if micros == 0
            || micros > MAX_LATENCY_US.into()
            || self.histogram.record(micros as u64).is_err()
        {
            add(&mut self.out_of_range, 1, overflow);
        }
    }

    pub(crate) fn snapshot(&self) -> LatencySnapshot {
        let histogram = &self.histogram;
        let populated = !histogram.is_empty();
        let quantile = |value| populated.then(|| histogram.value_at_quantile(value) as u32);
        LatencySnapshot {
            significant_digits: 3,
            lowest_us: 1,
            highest_us: MAX_LATENCY_US as u32,
            samples: histogram.len(),
            out_of_range: self.out_of_range,
            mean_us: populated.then(|| histogram.mean()),
            min_us: populated.then(|| histogram.min() as u32),
            max_us: populated.then(|| histogram.max() as u32),
            median_us: quantile(0.5),
            p90_us: quantile(0.9),
            p95_us: quantile(0.95),
            p99_us: quantile(0.99),
        }
    }
}

fn add(counter: &mut u64, amount: u64, overflow: &mut bool) {
    match counter.checked_add(amount) {
        Some(value) => *counter = value,
        None => {
            *counter = u64::MAX;
            *overflow = true;
        }
    }
}

pub(crate) struct Metrics {
    scenario_mode: bool,
    scenario_checks: u64,
    scenario_check_failures: u64,
    scenario_checks_unavailable: bool,
    diagnostics: crate::ResponseDiagnostics,
    counters: Counters,
    response: Distribution,
    interval_response: Distribution,
    interval_requests: u64,
    interval_failures: u64,
    transport: Distribution,
    timeout: Distribution,
    wait_us: u64,
    interference: bool,
    overflow: bool,
    max_in_flight: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            scenario_mode: false,
            scenario_checks: 0,
            scenario_check_failures: 0,
            scenario_checks_unavailable: false,
            diagnostics: crate::ResponseDiagnostics::new(&[], crate::InspectionLimits::default()),
            counters: Counters::default(),
            response: Distribution::new(),
            interval_response: Distribution::new(),
            interval_requests: 0,
            interval_failures: 0,
            transport: Distribution::new(),
            timeout: Distribution::new(),
            wait_us: 0,
            interference: false,
            overflow: false,
            max_in_flight: 0,
        }
    }

    pub(crate) fn configured(
        assertions: &[crate::Assertion],
        limits: crate::InspectionLimits,
    ) -> Self {
        Self {
            diagnostics: crate::ResponseDiagnostics::new(assertions, limits),
            ..Self::new()
        }
    }

    pub(crate) fn inspect(
        &mut self,
        observation: &crate::assertions::Observation,
        outcome: Outcome,
    ) {
        self.diagnostics
            .observe(observation, matches!(outcome, Outcome::Response(_)));
    }

    pub(crate) fn scenario_checks(
        &mut self,
        observation: &crate::assertions::Observation,
        outcome: Outcome,
    ) {
        for check in &observation.checks {
            add(&mut self.scenario_checks, 1, &mut self.overflow);
            match check {
                crate::CheckOutcome::Passed => {}
                crate::CheckOutcome::StatusMismatch
                | crate::CheckOutcome::MissingHeader
                | crate::CheckOutcome::HeaderMismatch
                | crate::CheckOutcome::MissingField
                | crate::CheckOutcome::JsonMismatch
                | crate::CheckOutcome::TextMismatch => {
                    add(&mut self.scenario_check_failures, 1, &mut self.overflow);
                }
                _ => self.scenario_checks_unavailable = true,
            }
        }
        if !matches!(outcome, Outcome::Response(_)) {
            self.scenario_checks_unavailable = true;
        }
    }

    pub(crate) fn scenarios(&mut self) {
        self.scenario_mode = true;
    }

    pub(crate) fn iteration(&mut self, failed: bool) {
        add(
            &mut self.counters.completed_iterations,
            1,
            &mut self.overflow,
        );
        if failed {
            add(&mut self.counters.failed_iterations, 1, &mut self.overflow);
        }
    }

    pub fn admission(&mut self) {
        add(&mut self.counters.started, 1, &mut self.overflow);
        add(&mut self.counters.in_flight, 1, &mut self.overflow);
        self.max_in_flight = self.max_in_flight.max(self.counters.in_flight);
    }

    pub fn wait(&mut self, duration: Duration) {
        add(
            &mut self.wait_us,
            duration.as_micros().min(u64::MAX.into()) as u64,
            &mut self.overflow,
        );
        self.interference |= duration > Duration::from_millis(1);
    }

    pub fn observe(&mut self, outcome: Outcome, elapsed: Duration, bytes: u64) {
        let counters = &mut self.counters;
        counters.in_flight -= 1;
        add(&mut counters.response_body_bytes, bytes, &mut self.overflow);
        match outcome {
            Outcome::Response(success) => {
                add(&mut counters.responses, 1, &mut self.overflow);
                if success {
                    add(&mut counters.successes, 1, &mut self.overflow);
                } else {
                    add(&mut counters.status_failures, 1, &mut self.overflow);
                    if !self.scenario_mode {
                        add(&mut counters.failed_iterations, 1, &mut self.overflow);
                    }
                }
                self.response.record(elapsed, &mut self.overflow);
                self.interval_response.record(elapsed, &mut self.overflow);
            }
            Outcome::Transport => {
                add(&mut counters.transport_failures, 1, &mut self.overflow);
                if !self.scenario_mode {
                    add(&mut counters.failed_iterations, 1, &mut self.overflow);
                }
                self.transport.record(elapsed, &mut self.overflow);
            }
            Outcome::Timeout => {
                add(&mut counters.timeouts, 1, &mut self.overflow);
                if !self.scenario_mode {
                    add(&mut counters.failed_iterations, 1, &mut self.overflow);
                }
                self.timeout.record(elapsed, &mut self.overflow);
            }
            Outcome::Cancelled => add(&mut counters.cancelled, 1, &mut self.overflow),
        }
        if !matches!(outcome, Outcome::Cancelled) {
            if !self.scenario_mode {
                add(&mut counters.completed_iterations, 1, &mut self.overflow);
            }
            add(&mut self.interval_requests, 1, &mut self.overflow);
            if !matches!(outcome, Outcome::Response(true)) {
                add(&mut self.interval_failures, 1, &mut self.overflow);
            }
        }
    }

    pub fn take_interval(&mut self) -> (LatencySnapshot, u64, u64) {
        let latency = self.interval_response.snapshot();
        self.interval_response.histogram.reset();
        self.interval_response.out_of_range = 0;
        let requests = std::mem::take(&mut self.interval_requests);
        let failures = std::mem::take(&mut self.interval_failures);
        (latency, requests, failures)
    }

    pub fn snapshot(&self, elapsed: Duration) -> MetricsSnapshot {
        let counters = &self.counters;
        let terminal = counters.responses as f64
            + counters.transport_failures as f64
            + counters.timeouts as f64;
        let failures = counters.status_failures as f64
            + counters.transport_failures as f64
            + counters.timeouts as f64;
        let seconds = elapsed.as_secs_f64();
        MetricsSnapshot {
            diagnostics: self.diagnostics.clone(),
            assertion_failure_rate: if self.scenario_mode {
                (self.scenario_checks > 0 && !self.scenario_checks_unavailable)
                    .then_some(self.scenario_check_failures as f64 / self.scenario_checks as f64)
            } else {
                self.diagnostics.failure_rate()
            },
            counters: counters.clone(),
            response_latency: self.response.snapshot(),
            transport_latency: self.transport.snapshot(),
            timeout_latency: self.timeout.snapshot(),
            http_failure_rate: (terminal > 0.0).then_some(failures / terminal),
            request_rate: if seconds > 0.0 {
                terminal / seconds
            } else {
                0.0
            },
            iteration_rate: if seconds > 0.0 {
                counters.completed_iterations as f64 / seconds
            } else {
                0.0
            },
            success_rate: if seconds > 0.0 {
                counters.successes as f64 / seconds
            } else {
                0.0
            },
            aggregation_wait_us: self.wait_us,
            aggregation_interference: self.interference,
            counter_overflow: self.overflow,
            max_in_flight: self.max_in_flight,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn populations_range_accounting_and_wire_precision() {
        let mut metrics = Metrics::new();
        for (outcome, micros) in [
            (Outcome::Response(true), 10),
            (Outcome::Response(false), 30),
            (Outcome::Transport, 50),
            (Outcome::Timeout, 70),
            (Outcome::Response(true), 0),
            (Outcome::Response(true), MAX_LATENCY_US + 1),
            (Outcome::Cancelled, 10),
        ] {
            metrics.admission();
            metrics.observe(outcome, Duration::from_micros(micros), 12);
        }
        let snapshot = metrics.snapshot(Duration::from_secs(2));
        assert_eq!(snapshot.response_latency.samples, 2);
        assert_eq!(snapshot.response_latency.out_of_range, 2);
        assert_eq!(snapshot.transport_latency.samples, 1);
        assert_eq!(snapshot.timeout_latency.samples, 1);
        assert_eq!(snapshot.counters.in_flight, 0);
        assert_eq!(snapshot.counters.completed_iterations, 6);
        assert_eq!(snapshot.counters.failed_iterations, 3);
        assert_eq!(snapshot.http_failure_rate, Some(0.5));
        assert_eq!(snapshot.request_rate, 3.0);
        metrics.counters.started = 9_007_199_254_740_993;
        let value = serde_json::to_value(metrics.snapshot(Duration::ZERO)).unwrap();
        assert_eq!(value["counters"]["started"], "9007199254740993");
        assert_eq!(value["counters"]["response_body_bytes"], "84");
    }

    #[test]
    fn interval_populations_reset_without_changing_cumulative_accounting() {
        let mut metrics = Metrics::new();
        for outcome in [
            Outcome::Response(true),
            Outcome::Response(false),
            Outcome::Transport,
            Outcome::Timeout,
            Outcome::Cancelled,
        ] {
            metrics.admission();
            metrics.observe(outcome, Duration::from_micros(25), 0);
        }
        let (latency, requests, failures) = metrics.take_interval();
        assert_eq!(requests, 4);
        assert_eq!(failures, 3);
        assert_eq!(latency.samples, 2);
        assert_eq!(latency.p95_us, Some(25));
        let (empty, requests, failures) = metrics.take_interval();
        assert_eq!(requests, 0);
        assert_eq!(failures, 0);
        assert_eq!(empty.samples, 0);
        assert!(empty.p95_us.is_none());
        let cumulative = metrics.snapshot(Duration::from_secs(1));
        assert_eq!(cumulative.counters.started, 5);
        assert_eq!(cumulative.counters.completed_iterations, 4);
        assert_eq!(cumulative.response_latency.samples, 2);
        assert_eq!(cumulative.http_failure_rate, Some(0.75));
    }

    #[test]
    fn empty_overflow_and_interference_are_visible() {
        let mut metrics = Metrics::new();
        assert!(
            metrics
                .snapshot(Duration::ZERO)
                .response_latency
                .p99_us
                .is_none()
        );
        assert!(metrics.snapshot(Duration::ZERO).http_failure_rate.is_none());
        metrics.counters.started = u64::MAX;
        metrics.admission();
        metrics.wait(Duration::from_millis(2));
        let snapshot = metrics.snapshot(Duration::ZERO);
        assert!(snapshot.counter_overflow);
        assert!(snapshot.aggregation_interference);
    }
}

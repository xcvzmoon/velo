use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{MetricsSnapshot, RunError};

/// Final cumulative metric and its display unit.
///
/// # Examples
/// ```
/// assert_eq!(serde_json::to_value(velo_engine::ThresholdMetric::ResponseP95Ms)?, "response_p95_ms");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdMetric {
    /// Approximate p95 of completed HTTP responses, in milliseconds.
    ResponseP95Ms,
    /// Status, transport and timeout failures over non-cancelled attempts, in percent.
    HttpFailurePercent,
    /// Failed assertion evaluations in percent; unavailable checks make this insufficient.
    AssertionFailurePercent,
    /// Non-cancelled terminal attempts per actual second, including drain.
    RequestRate,
}

/// Strict comparison; equality always fails.
///
/// # Examples
/// ```
/// assert_eq!(serde_json::to_value(velo_engine::ThresholdOperator::LessThan)?, "less_than");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdOperator {
    /// Observed value must be strictly below the limit.
    LessThan,
    /// Observed value must be strictly above the limit.
    GreaterThan,
}

/// One final metric rule. Limits are finite, nonnegative, and use the metric's unit.
///
/// # Examples
/// ```
/// let rule = velo_engine::Threshold {
///     metric: velo_engine::ThresholdMetric::ResponseP95Ms,
///     operator: velo_engine::ThresholdOperator::LessThan,
///     limit: 500.0,
/// };
/// assert_eq!(rule.limit, 500.0);
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Threshold {
    /// Population and unit to evaluate.
    pub metric: ThresholdMetric,
    /// Strict comparison against the limit.
    pub operator: ThresholdOperator,
    /// Maximum 100 for percentages and 3,600,000 for latency milliseconds.
    pub limit: f64,
}

/// Individual final verdict, separate from execution lifecycle and HTTP outcomes.
///
/// # Examples
/// ```
/// assert_eq!(serde_json::to_value(velo_engine::ThresholdStatus::InsufficientData)?, "insufficient_data");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdStatus {
    /// The observed metric satisfies the strict comparison.
    Passed,
    /// The observed metric does not satisfy the strict comparison.
    Failed,
    /// Required data is absent or incomplete; this rule cannot pass.
    InsufficientData,
}

/// Rule and observed final value. The containing run retains its partial-run context.
///
/// # Examples
/// ```
/// fn passed(result: &velo_engine::ThresholdResult) -> bool {
///     result.status == velo_engine::ThresholdStatus::Passed
/// }
/// ```
#[derive(Clone, Debug, Serialize, TS)]
pub struct ThresholdResult {
    /// Configured rule in its original order.
    pub rule: Threshold,
    /// Unrounded observed value in the rule's unit, or None for insufficient data.
    pub observed: Option<f64>,
    /// Final comparison outcome.
    pub status: ThresholdStatus,
}

pub(crate) fn validate(thresholds: &[Threshold]) -> Result<(), RunError> {
    for threshold in thresholds {
        let maximum = match threshold.metric {
            ThresholdMetric::ResponseP95Ms => 3_600_000.0,
            ThresholdMetric::HttpFailurePercent | ThresholdMetric::AssertionFailurePercent => 100.0,
            ThresholdMetric::RequestRate => f64::MAX,
        };
        if !threshold.limit.is_finite() || !(0.0..=maximum).contains(&threshold.limit) {
            return Err(RunError::new(
                "INVALID_RUN_CONFIG",
                "Threshold limits must be finite, nonnegative, and within the metric range.",
                Some("thresholds"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn evaluate(
    thresholds: &[Threshold],
    metrics: &MetricsSnapshot,
) -> Vec<ThresholdResult> {
    thresholds
        .iter()
        .map(|rule| {
            let observed = if metrics.counter_overflow {
                None
            } else {
                match rule.metric {
                    ThresholdMetric::ResponseP95Ms
                        if metrics.response_latency.out_of_range == 0 =>
                    {
                        metrics
                            .response_latency
                            .p95_us
                            .map(|value| f64::from(value) / 1000.0)
                    }
                    ThresholdMetric::ResponseP95Ms => None,
                    ThresholdMetric::HttpFailurePercent => {
                        metrics.http_failure_rate.map(|value| value * 100.0)
                    }
                    ThresholdMetric::AssertionFailurePercent => {
                        metrics.assertion_failure_rate.map(|value| value * 100.0)
                    }
                    ThresholdMetric::RequestRate => metrics
                        .http_failure_rate
                        .and_then(|_| (metrics.request_rate > 0.0).then_some(metrics.request_rate)),
                }
            }
            .filter(|value| value.is_finite());
            let status = match observed {
                None => ThresholdStatus::InsufficientData,
                Some(value) => {
                    let passed = match rule.operator {
                        ThresholdOperator::LessThan => value < rule.limit,
                        ThresholdOperator::GreaterThan => value > rule.limit,
                    };
                    if passed {
                        ThresholdStatus::Passed
                    } else {
                        ThresholdStatus::Failed
                    }
                }
            };
            ThresholdResult {
                rule: rule.clone(),
                observed,
                status,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::metrics::{Metrics, Outcome};

    use super::*;

    fn rule(metric: ThresholdMetric, operator: ThresholdOperator, limit: f64) -> Threshold {
        Threshold {
            metric,
            operator,
            limit,
        }
    }

    #[test]
    fn strict_boundaries_and_metric_units() {
        let mut metrics = Metrics::new();
        metrics.admission();
        metrics.observe(Outcome::Response(false), Duration::from_micros(500), 0);
        let snapshot = metrics.snapshot(Duration::from_secs(2));
        for (metric, value) in [
            (ThresholdMetric::ResponseP95Ms, 0.5),
            (ThresholdMetric::HttpFailurePercent, 100.0),
            (ThresholdMetric::RequestRate, 0.5),
        ] {
            for operator in [ThresholdOperator::LessThan, ThresholdOperator::GreaterThan] {
                let results = evaluate(&[rule(metric, operator, value)], &snapshot);
                assert_eq!(results[0].observed, Some(value));
                assert_eq!(results[0].status, ThresholdStatus::Failed);
            }
            assert_eq!(
                evaluate(
                    &[rule(metric, ThresholdOperator::LessThan, value + 1.0)],
                    &snapshot
                )[0]
                .status,
                ThresholdStatus::Passed
            );
            assert_eq!(
                evaluate(
                    &[rule(metric, ThresholdOperator::GreaterThan, value - 0.1)],
                    &snapshot
                )[0]
                .status,
                ThresholdStatus::Passed
            );
        }
    }

    #[test]
    fn empty_cancelled_and_incomplete_data_cannot_pass() {
        let rules = [
            rule(
                ThresholdMetric::ResponseP95Ms,
                ThresholdOperator::LessThan,
                500.0,
            ),
            rule(
                ThresholdMetric::HttpFailurePercent,
                ThresholdOperator::LessThan,
                1.0,
            ),
            rule(
                ThresholdMetric::RequestRate,
                ThresholdOperator::GreaterThan,
                0.0,
            ),
        ];
        let mut metrics = Metrics::new();
        for cancelled in [false, true] {
            if cancelled {
                metrics.admission();
                metrics.observe(Outcome::Cancelled, Duration::from_millis(1), 0);
            }
            assert!(
                evaluate(&rules, &metrics.snapshot(Duration::from_secs(1)))
                    .iter()
                    .all(|result| result.status == ThresholdStatus::InsufficientData)
            );
        }
        metrics.admission();
        metrics.observe(Outcome::Response(true), Duration::ZERO, 0);
        let mut snapshot = metrics.snapshot(Duration::from_secs(1));
        assert_eq!(
            evaluate(&rules, &snapshot)[0].status,
            ThresholdStatus::InsufficientData
        );
        assert_eq!(
            evaluate(&rules, &snapshot)[1].status,
            ThresholdStatus::Passed
        );
        snapshot.counter_overflow = true;
        assert!(
            evaluate(&rules, &snapshot)
                .iter()
                .all(|result| result.status == ThresholdStatus::InsufficientData)
        );
    }

    #[test]
    fn rejects_invalid_limits_and_accounts_for_retained_rules() {
        for (metric, limit) in [
            (ThresholdMetric::RequestRate, f64::NAN),
            (ThresholdMetric::RequestRate, f64::INFINITY),
            (ThresholdMetric::RequestRate, -1.0),
            (ThresholdMetric::HttpFailurePercent, 100.1),
            (ThresholdMetric::ResponseP95Ms, 3_600_001.0),
        ] {
            let config = crate::RunConfig {
                url: "http://127.0.0.1/".into(),
                thresholds: vec![rule(metric, ThresholdOperator::LessThan, limit)],
                ..Default::default()
            };
            let error = crate::RunManager::prepare(config).unwrap_err();
            assert_eq!(error.code, "INVALID_RUN_CONFIG");
            assert_eq!(error.field.as_deref(), Some("thresholds"));
        }
        let config = crate::RunConfig {
            url: "http://127.0.0.1/".into(),
            thresholds: vec![
                rule(
                    ThresholdMetric::RequestRate,
                    ThresholdOperator::GreaterThan,
                    1.0
                );
                100
            ],
            budget: crate::ResourceBudget {
                max_request_bytes: 100,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            crate::RunManager::prepare(config).unwrap_err().code,
            "REQUEST_BUDGET_EXCEEDED"
        );
    }
}

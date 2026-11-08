//! Baseline comparison and portable reports over redacted historical measurements.
//!
//! Percentage changes use unrounded values; positive means candidate minus baseline.
//! Budgets bound worsening: throughput decreases, while latency and failure rates increase.
//! No statistical significance is inferred. Configuration changes require explicit review.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod export;

#[doc(inline)]
pub use export::{export_html, export_json, export_junit};
use ts_rs::TS;

use crate::{CommandError, HistoryEntry};

/// Editable release context; never used to alter measured values.
///
/// # Examples
/// ```
/// let metadata = velo_contracts::reports::RunMetadata {
///     git_sha: "a1b2c3d".into(), release: "v1.2".into(), ..Default::default()
/// };
/// velo_contracts::reports::validate_metadata(&metadata)?;
/// # Ok::<(), velo_contracts::CommandError>(())
/// ```
#[derive(Clone, Debug, Default, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RunMetadata {
    /// Optional Git SHA, empty when unset.
    pub git_sha: String,
    /// Release label, empty when unset.
    pub release: String,
    /// At most 20 user labels.
    #[ts(type = "string[]")]
    pub tags: Vec<String>,
    /// User notes; include only information safe to share.
    pub notes: String,
}

/// Maximum permitted worsening in percent or absolute metric units.
///
/// # Examples
/// ```
/// use velo_contracts::reports::{RegressionBudget, RegressionMetric};
/// let budget = RegressionBudget { metric: RegressionMetric::RequestRate, limit: 10.0, percentage: true };
/// assert_eq!(budget.limit, 10.0);
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RegressionBudget {
    /// Metric population and unit.
    pub metric: RegressionMetric,
    /// Finite nonnegative limit; equality passes.
    pub limit: f64,
    /// Whether the limit uses relative percent instead of absolute units.
    pub percentage: bool,
}

/// Stored baseline dependency and reviewed regression rules.
///
/// # Examples
/// ```
/// use velo_contracts::reports::{ComparisonSettings, validate_settings};
/// let settings = ComparisonSettings { baseline_id: "release-1".into(), budgets: vec![], configuration_reviewed: false };
/// validate_settings(&settings)?;
/// # Ok::<(), velo_contracts::CommandError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ComparisonSettings {
    /// Durable reference; deletion does not select a replacement.
    pub baseline_id: String,
    /// At most 32 regression limits, independent of absolute thresholds.
    #[ts(type = "RegressionBudget[]")]
    pub budgets: Vec<RegressionBudget>,
    /// Explicit acknowledgement of the displayed configuration differences.
    pub configuration_reviewed: bool,
}

/// One changed safe configuration field.
///
/// # Examples
/// ```
/// use velo_contracts::reports::ConfigurationDifference;
/// let difference = ConfigurationDifference {
///     field: "duration_ms".into(), baseline: serde_json::json!(1000), candidate: serde_json::json!(2000),
/// };
/// assert_ne!(difference.baseline, difference.candidate);
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
pub struct ConfigurationDifference {
    /// Name of the changed configuration field.
    pub field: String,
    /// Original safe value.
    #[ts(type = "JsonValue")]
    pub baseline: Value,
    /// Candidate safe value.
    #[ts(type = "JsonValue")]
    pub candidate: Value,
}

/// Unrounded observed change for one evaluated population.
///
/// # Examples
/// ```
/// use velo_contracts::reports::{MetricDelta, RegressionMetric};
/// let delta = MetricDelta {
///     metric: RegressionMetric::HttpFailurePercent, baseline: Some(0.0), candidate: Some(1.0),
///     absolute: Some(1.0), percentage: None,
/// };
/// assert!(delta.percentage.is_none());
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
pub struct MetricDelta {
    /// Metric population and unit.
    pub metric: RegressionMetric,
    /// Baseline measurement, absent when unavailable.
    pub baseline: Option<f64>,
    /// Candidate measurement, absent when unavailable.
    pub candidate: Option<f64>,
    /// Candidate minus baseline in metric units.
    pub absolute: Option<f64>,
    /// 100 times delta divided by baseline; absent for zero or missing baseline.
    pub percentage: Option<f64>,
}

/// Individual budget outcome with its evidence.
///
/// # Examples
/// ```
/// fn exceeded_budget(result: &velo_contracts::reports::RegressionResult) -> bool {
///     result.verdict == velo_contracts::reports::RegressionVerdict::Fail
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
pub struct RegressionResult {
    /// Configured limit.
    pub budget: RegressionBudget,
    /// Direction-adjusted change in the budget unit.
    pub worsening: Option<f64>,
    /// Pass, fail, or unevaluable; never an absolute threshold verdict.
    pub verdict: RegressionVerdict,
}

/// Versioned evidence retaining the baseline dependency and candidate context.
///
/// # Examples
/// ```
/// fn needs_review(report: &velo_contracts::reports::ComparisonReport) -> bool {
///     !report.configuration_reviewed || !report.limitations.is_empty()
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ComparisonReport {
    /// Report schema version, currently 1.
    pub format_version: u32,
    /// Requested baseline identifier even after deletion.
    pub baseline_id: String,
    /// Baseline evidence, absent after deletion.
    pub baseline: Option<HistoryEntry>,
    /// Candidate measurements and release context.
    pub candidate: HistoryEntry,
    /// Whether the user explicitly reviewed retained and redacted configuration context.
    pub configuration_reviewed: bool,
    /// Safe configuration differences requiring review.
    #[ts(type = "ConfigurationDifference[]")]
    pub differences: Vec<ConfigurationDifference>,
    /// Compatibility, completeness, and integrity limitations.
    #[ts(type = "string[]")]
    pub limitations: Vec<String>,
    /// Unrounded metric changes, independent of budgets.
    #[ts(type = "MetricDelta[]")]
    pub deltas: Vec<MetricDelta>,
    /// Budget verdicts, separate from thresholds.
    #[ts(type = "RegressionResult[]")]
    pub regressions: Vec<RegressionResult>,
    /// Combined regression outcome; no configured budgets is unevaluable.
    pub verdict: RegressionVerdict,
}

/// Supported evaluated metrics with fixed units and directionality.
///
/// # Examples
/// ```
/// use velo_contracts::reports::RegressionMetric;
/// assert_eq!(serde_json::to_value(RegressionMetric::ResponseP95Ms)?, "response_p95_ms");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RegressionMetric {
    /// Completed-response HDR p50 in milliseconds; lower is better.
    ResponseP50Ms,
    /// Completed-response HDR p95 in milliseconds; lower is better.
    ResponseP95Ms,
    /// Completed-response HDR p99 in milliseconds; lower is better.
    ResponseP99Ms,
    /// Completed non-cancelled requests per evaluated second; higher is better.
    RequestRate,
    /// HTTP failures over non-cancelled attempts in percent; lower is better.
    HttpFailurePercent,
    /// Failed assertion evaluations in percent; lower is better.
    AssertionFailurePercent,
}

/// Regression outcome independent of execution state and absolute thresholds.
///
/// # Examples
/// ```
/// use velo_contracts::reports::RegressionVerdict;
/// assert_eq!(serde_json::to_value(RegressionVerdict::Unevaluable)?, "unevaluable");
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RegressionVerdict {
    /// Every configured limit was evaluable and satisfied.
    Pass,
    /// At least one evaluable limit was exceeded.
    Fail,
    /// No budgets, missing data, incompatible runs, or unreviewed differences.
    Unevaluable,
}

fn invalid(message: &str) -> CommandError {
    CommandError {
        code: "INVALID_REPORT_SETTINGS".into(),
        message: message.into(),
        field: None,
    }
}

/// Validates editable metadata without touching measurements.
///
/// # Arguments
/// * `metadata` - User-authored release context, with no inferred request content.
/// # Returns
/// Unit when the bounded fields are valid.
/// # Errors
/// Returns `INVALID_REPORT_SETTINGS` for invalid SHA or oversized fields.
/// # Examples
/// ```
/// velo_contracts::reports::validate_metadata(&Default::default())?;
/// # Ok::<(), velo_contracts::CommandError>(())
/// ```
pub fn validate_metadata(metadata: &RunMetadata) -> Result<(), CommandError> {
    if metadata.release.len() > 120
        || metadata.notes.len() > 4000
        || metadata.tags.len() > 20
        || metadata
            .tags
            .iter()
            .any(|tag| tag.is_empty() || tag.len() > 64)
        || (!metadata.git_sha.is_empty()
            && (!(7..=64).contains(&metadata.git_sha.len())
                || !metadata
                    .git_sha
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())))
    {
        return Err(invalid(
            "Use a 7–64 digit hexadecimal SHA, a release up to 120 bytes, 20 tags up to 64 bytes each, and notes up to 4,000 bytes.",
        ));
    }
    Ok(())
}

/// Validates persisted regression settings at the command or CLI boundary.
///
/// # Arguments
/// * `settings` - Baseline dependency, budgets, and explicit configuration review.
/// # Returns
/// Unit when all budgets and the baseline identifier are bounded.
/// # Errors
/// Returns `INVALID_REPORT_SETTINGS` for empty identifiers or nonfinite, negative, or excessive limits.
/// # Examples
/// ```
/// use velo_contracts::reports::{ComparisonSettings, validate_settings};
/// validate_settings(&ComparisonSettings { baseline_id: "baseline".into(), budgets: vec![], configuration_reviewed: false })?;
/// # Ok::<(), velo_contracts::CommandError>(())
/// ```
pub fn validate_settings(settings: &ComparisonSettings) -> Result<(), CommandError> {
    if settings.baseline_id.is_empty()
        || settings.baseline_id.len() > 128
        || settings.budgets.len() > 32
        || settings
            .budgets
            .iter()
            .any(|budget| !budget.limit.is_finite() || budget.limit < 0.0 || budget.limit > 1e12)
    {
        return Err(invalid(
            "Choose a baseline and at most 32 finite, nonnegative budgets up to 1e12.",
        ));
    }
    Ok(())
}

fn observed(entry: &HistoryEntry, metric: RegressionMetric) -> Option<f64> {
    let metrics = &entry.snapshot["evaluated"];
    if entry.snapshot["measurement"]["evaluated_us"]
        .as_str()
        .and_then(|value| value.parse::<u64>().ok())
        .is_none_or(|elapsed| elapsed == 0)
    {
        return None;
    }
    if metrics["counter_overflow"] != false {
        return None;
    }
    let value = match metric {
        RegressionMetric::ResponseP50Ms
        | RegressionMetric::ResponseP95Ms
        | RegressionMetric::ResponseP99Ms => {
            let latency = &metrics["response_latency"];
            if latency["samples"]
                .as_str()
                .and_then(|value| value.parse::<u64>().ok())
                .is_none_or(|samples| samples == 0)
            {
                return None;
            }
            if latency["out_of_range"] != "0" {
                return None;
            }
            let key = match metric {
                RegressionMetric::ResponseP50Ms => "median_us",
                RegressionMetric::ResponseP95Ms => "p95_us",
                _ => "p99_us",
            };
            latency[key].as_f64().map(|value| value / 1000.0)
        }
        RegressionMetric::RequestRate => metrics["http_failure_rate"]
            .as_f64()
            .and_then(|_| metrics["request_rate"].as_f64()),
        RegressionMetric::HttpFailurePercent => metrics["http_failure_rate"]
            .as_f64()
            .map(|value| value * 100.0),
        RegressionMetric::AssertionFailurePercent => metrics["assertion_failure_rate"]
            .as_f64()
            .map(|value| value * 100.0),
    };
    value.filter(|value| value.is_finite() && *value >= 0.0)
}

fn limitations(entry: &HistoryEntry, label: &str, output: &mut Vec<String>) {
    let snapshot = &entry.snapshot;
    if entry.interrupted
        || entry.incomplete
        || snapshot["state"] != "completed"
        || snapshot["measurement"]["complete"] != true
    {
        output.push(format!(
            "{label}: interrupted, incomplete, or not completed; regression verdict unavailable."
        ));
    }
    for population in ["metrics", "evaluated"] {
        let metrics = &snapshot[population];
        if metrics["counter_overflow"] != false
            || metrics["aggregation_interference"] != false
            || ["response_latency", "transport_latency", "timeout_latency"]
                .iter()
                .any(|key| metrics[key]["out_of_range"] != "0")
        {
            output.push(format!(
                "{label}: measurement integrity is limited; regression verdict unavailable."
            ));
        }
    }
    if snapshot["arrival"]["capacity_drops"] != "0"
        || snapshot["arrival"]["missed_deadlines"] != "0"
        || snapshot["dataset"]["exhausted"] == true
    {
        output.push(format!(
            "{label}: generator admission drops or dataset exhaustion limit the comparison."
        ));
    }
}

/// Builds a baseline comparison from safe history without mutating measurements.
///
/// # Arguments
/// * `baseline` - Referenced historical run, or `None` when deleted or unavailable.
/// * `candidate` - Candidate history, including sampling and incomplete indicators.
/// * `settings` - Validated budgets and configuration acknowledgement.
/// # Returns
/// A version 1 report; incompatible or incomplete inputs cannot yield a clean pass.
/// # Examples
/// ```
/// # fn example(candidate: velo_contracts::HistoryEntry, settings: &velo_contracts::reports::ComparisonSettings) {
/// let report = velo_contracts::reports::compare(None, candidate, settings);
/// assert_eq!(report.verdict, velo_contracts::reports::RegressionVerdict::Unevaluable);
/// # }
/// ```
pub fn compare(
    baseline: Option<HistoryEntry>,
    candidate: HistoryEntry,
    settings: &ComparisonSettings,
) -> ComparisonReport {
    let mut warnings = Vec::new();
    if let Err(error) = validate_settings(settings) {
        warnings.push(error.message);
    }
    let mut differences = Vec::new();
    limitations(&candidate, "Candidate", &mut warnings);
    if let Some(baseline) = &baseline {
        limitations(baseline, "Baseline", &mut warnings);
        let origin = baseline.snapshot["confirmation"]["origin"].as_str();
        let workload = baseline.snapshot["confirmation"]["workload"]["kind"].as_str();
        if origin.is_none()
            || workload.is_none()
            || baseline.id != settings.baseline_id
            || baseline.id == candidate.id
            || baseline.method != candidate.method
            || baseline.snapshot["confirmation"]["origin"]
                != candidate.snapshot["confirmation"]["origin"]
            || baseline.snapshot["confirmation"]["workload"]["kind"]
                != candidate.snapshot["confirmation"]["workload"]["kind"]
        {
            warnings.push("Comparison blocked: choose distinct runs with matching origin, method, and workload kind.".into());
        }
        for field in [
            "origin",
            "workload",
            "duration_ms",
            "warmup_ms",
            "budget",
            "dataset",
            "abort_policies",
        ] {
            let mut left = baseline.snapshot["confirmation"][field].clone();
            let mut right = candidate.snapshot["confirmation"][field].clone();
            if field == "dataset" {
                for value in [&mut left, &mut right] {
                    if let Some(fields) = value.as_object_mut() {
                        for key in ["allocated", "cycles", "exhausted"] {
                            fields.remove(key);
                        }
                    }
                }
            }
            if left != right {
                differences.push(ConfigurationDifference {
                    field: field.into(),
                    baseline: left,
                    candidate: right,
                });
            }
        }
        if baseline.configuration != candidate.configuration {
            differences.push(ConfigurationDifference {
                field: "execution_settings".into(),
                baseline: baseline.configuration.clone(),
                candidate: candidate.configuration.clone(),
            });
        }
        if baseline.definition_identity != candidate.definition_identity {
            differences.push(ConfigurationDifference {
                field: "definition_identity".into(),
                baseline: json!(baseline.definition_identity),
                candidate: json!(candidate.definition_identity),
            });
        }
        if baseline.definition_identity.is_none() || candidate.definition_identity.is_none() {
            warnings.push("Definition identity is unavailable in this history; regression verdict unavailable.".into());
        }
        if !settings.configuration_reviewed {
            warnings.push(
                "Review configuration differences before accepting regression conclusions.".into(),
            );
        }
    } else {
        warnings.push(
            "Baseline unavailable: the referenced run was deleted or could not be found.".into(),
        );
    }
    let metrics = [
        RegressionMetric::ResponseP50Ms,
        RegressionMetric::ResponseP95Ms,
        RegressionMetric::ResponseP99Ms,
        RegressionMetric::RequestRate,
        RegressionMetric::HttpFailurePercent,
        RegressionMetric::AssertionFailurePercent,
    ];
    let deltas: Vec<_> = metrics
        .into_iter()
        .map(|metric| {
            let left = baseline.as_ref().and_then(|entry| observed(entry, metric));
            let right = observed(&candidate, metric);
            let absolute = left
                .zip(right)
                .map(|(left, right)| right - left)
                .filter(|value| value.is_finite());
            let percentage = left
                .zip(absolute)
                .and_then(|(left, delta)| (left != 0.0).then(|| delta / left * 100.0))
                .filter(|value| value.is_finite());
            MetricDelta {
                metric,
                baseline: left,
                candidate: right,
                absolute,
                percentage,
            }
        })
        .collect();
    let regressions: Vec<_> = settings
        .budgets
        .iter()
        .map(|budget| {
            let delta = deltas.iter().find(|delta| delta.metric == budget.metric);
            let worsening = delta
                .and_then(|delta| {
                    if budget.percentage {
                        delta.percentage
                    } else {
                        delta.absolute
                    }
                })
                .map(|value| {
                    if budget.metric == RegressionMetric::RequestRate {
                        -value
                    } else {
                        value
                    }
                });
            let verdict = if !warnings.is_empty() {
                RegressionVerdict::Unevaluable
            } else {
                match worsening {
                    Some(value) if value > budget.limit => RegressionVerdict::Fail,
                    Some(_) => RegressionVerdict::Pass,
                    None => RegressionVerdict::Unevaluable,
                }
            };
            RegressionResult {
                budget: budget.clone(),
                worsening,
                verdict,
            }
        })
        .collect();
    let verdict = if regressions
        .iter()
        .any(|result| result.verdict == RegressionVerdict::Fail)
    {
        RegressionVerdict::Fail
    } else if regressions.is_empty()
        || regressions
            .iter()
            .any(|result| result.verdict == RegressionVerdict::Unevaluable)
    {
        RegressionVerdict::Unevaluable
    } else {
        RegressionVerdict::Pass
    };
    ComparisonReport {
        format_version: 1,
        baseline_id: settings.baseline_id.clone(),
        configuration_reviewed: settings.configuration_reviewed,
        baseline,
        candidate,
        differences,
        limitations: warnings,
        deltas,
        regressions,
        verdict,
    }
}

/// Combines native outcomes without recalculating thresholds or assertion rules.
///
/// # Arguments
/// * `entry` - Content-free evidence from a terminal run.
/// # Returns
/// Fail for a breached rule, aborted admission, or failed scenario step. Incomplete,
/// corrupt, or empty measurements and unavailable checks are unevaluable. Otherwise pass.
/// Regression budgets remain separate and use [`compare`].
/// # Examples
/// ```
/// # fn example(entry: &velo_contracts::HistoryEntry) {
/// let release_allowed = velo_contracts::reports::run_verdict(entry)
///     == velo_contracts::reports::RegressionVerdict::Pass;
/// # let _ = release_allowed;
/// # }
/// ```
pub fn run_verdict(entry: &HistoryEntry) -> RegressionVerdict {
    let snapshot = &entry.snapshot;
    let mut insufficient = Vec::new();
    limitations(entry, "Run", &mut insufficient);
    let mut unavailable = !insufficient.is_empty();
    if snapshot["state"] == "aborted" {
        return RegressionVerdict::Fail;
    }
    let Some(thresholds) = snapshot["thresholds"].as_array() else {
        return RegressionVerdict::Unevaluable;
    };
    for threshold in thresholds {
        match threshold["status"].as_str() {
            Some("failed") => return RegressionVerdict::Fail,
            Some("passed") => {}
            _ => unavailable = true,
        }
    }
    let Some(assertions) = snapshot
        .pointer("/evaluated/diagnostics/assertions")
        .and_then(Value::as_array)
    else {
        return RegressionVerdict::Unevaluable;
    };
    for assertion in assertions {
        let counts = ["passed", "failed", "unavailable"].map(|key| {
            assertion[key]
                .as_str()
                .and_then(|value| value.parse::<u64>().ok())
        });
        match counts {
            [Some(_), Some(failed), Some(_)] if failed > 0 => return RegressionVerdict::Fail,
            [Some(passed), Some(0), Some(0)] if passed > 0 => {}
            _ => unavailable = true,
        }
    }
    if let Some(steps) = snapshot
        .pointer("/scenarios/steps")
        .and_then(Value::as_array)
    {
        for step in steps {
            if step["failed"]
                .as_str()
                .and_then(|value| value.parse::<u64>().ok())
                .is_some_and(|count| count > 0)
            {
                return RegressionVerdict::Fail;
            }
        }
    }
    if snapshot
        .pointer("/evaluated/counters/responses")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u64>().ok())
        .is_none_or(|count| count == 0)
    {
        unavailable = true;
    }
    if unavailable {
        RegressionVerdict::Unevaluable
    } else {
        RegressionVerdict::Pass
    }
}

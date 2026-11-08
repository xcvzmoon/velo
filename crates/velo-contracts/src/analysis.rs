//! Content-free measurement selection for explicit AI and MCP sharing.
//!
//! Input boundaries must limit encoded data before parsing. Selection caps nesting at
//! twelve levels and arrays at 128 entries, retaining only recognized measurement keys.

use serde_json::{Map, Value, json};

use crate::{HistoryEntry, reports::ComparisonReport};

const EVIDENCE_KEYS: &[&str] = &[
    "comparison_limited",
    "configuration_reviewed",
    "configuration_differences",
    "arrival",
    "candidate",
    "baseline",
    "metrics",
    "evaluated",
    "measurement",
    "generator",
    "telemetry",
    "incomplete",
    "interrupted",
    "stride",
    "deltas",
    "diagnostics",
    "assertions",
    "samples",
    "counters",
    "response_latency",
    "transport_latency",
    "timeout_latency",
    "assertion_failure_rate",
    "http_failure_rate",
    "request_rate",
    "iteration_rate",
    "success_rate",
    "aggregation_wait_us",
    "aggregation_interference",
    "counter_overflow",
    "max_in_flight",
    "started",
    "responses",
    "successes",
    "status_failures",
    "transport_failures",
    "timeouts",
    "cancelled",
    "in_flight",
    "response_body_bytes",
    "completed_iterations",
    "failed_iterations",
    "significant_digits",
    "lowest_us",
    "highest_us",
    "out_of_range",
    "mean_us",
    "min_us",
    "max_us",
    "median_us",
    "p90_us",
    "p95_us",
    "p99_us",
    "warmup_ms",
    "evaluated_us",
    "scheduling_us",
    "drain_us",
    "actual_us",
    "complete",
    "available",
    "passed",
    "failed",
    "unavailable",
    "evaluations",
    "check_index",
    "status",
    "count",
    "absolute",
    "percentage",
    "metric",
    "offered",
    "capacity_drops",
    "dataset_drops",
    "missed_deadlines",
    "max_lateness_us",
    "sampled_at_us",
    "interval_us",
    "logical_cpus",
    "process_cpu_percent",
    "process_memory_bytes",
    "system_cpu_percent",
    "system_available_memory_bytes",
    "system_total_memory_bytes",
    "start_us",
    "end_us",
    "empty",
    "warmup_complete",
    "evaluated_started",
    "elapsed_us",
    "duration_us",
];

fn select(value: &Value, depth: usize, field: &str) -> Value {
    if depth > 12 {
        return Value::Null;
    }
    match value {
        Value::Number(_) | Value::Bool(_) | Value::Null => value.clone(),
        Value::String(_) if field == "metric" => {
            serde_json::from_value::<crate::reports::RegressionMetric>(value.clone())
                .ok()
                .and_then(|metric| serde_json::to_value(metric).ok())
                .unwrap_or(Value::Null)
        }
        Value::String(text)
            if text.len() <= 20 && text.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            text.parse::<u64>()
                .map_or(Value::Null, |value| json!(value))
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .take(128)
                .map(|item| select(item, depth + 1, field))
                .collect(),
        ),
        Value::Object(items) => {
            let mut sanitized = Map::new();
            for (key, value) in items {
                if EVIDENCE_KEYS.contains(&key.as_str()) {
                    sanitized.insert(key.clone(), select(value, depth + 1, key));
                }
            }
            Value::Object(sanitized)
        }
        _ => Value::Null,
    }
}

/// Retains recognized numeric measurements and completeness flags while removing request content.
///
/// Strings become null except supported regression metric names and decimal unsigned counters.
/// Counter conversion preserves the full unsigned 64-bit range. Unknown object keys are removed;
/// selected arrays retain their first 128 entries and nesting beyond twelve levels becomes null.
/// The caller must bound encoded input before parsing; this function does not limit input bytes.
/// Selection does not authenticate measurements or establish their provenance.
///
/// # Arguments
/// * `value` - Bounded measurement context to select without resolving secrets or performing I/O.
///
/// # Returns
/// A new content-free JSON value. Missing or unsupported scalar values become null.
///
/// # Examples
/// ```
/// use serde_json::json;
/// let selected = velo_contracts::analysis::redact_evidence(&json!({
///     "metrics": {"request_rate": 12.0, "private-header": "secret"}
/// }));
/// assert_eq!(selected, json!({"metrics": {"request_rate": 12.0}}));
/// ```
pub fn redact_evidence(value: &Value) -> Value {
    select(value, 0, "")
}

fn measurements(entry: &HistoryEntry) -> Value {
    let snapshot = &entry.snapshot;
    json!({
        "metrics": snapshot["metrics"],
        "evaluated": snapshot["evaluated"],
        "measurement": snapshot["measurement"],
        "arrival": snapshot["arrival"],
        "telemetry": snapshot["telemetry"],
        "incomplete": entry.incomplete,
        "interrupted": entry.interrupted,
        "stride": entry.stride,
    })
}

/// Selects redacted candidate, baseline, and comparison measurements from a portable report.
///
/// Request configuration, user metadata, history timelines, diagnostic text, and report labels
/// are omitted. Comparison limitations remain visible as a boolean flag. The report may have
/// been imported; selection neither verifies its claims nor recomputes its derived values.
///
/// # Arguments
/// * `report` - A bounded version 1 report selected for sharing by the user.
///
/// # Returns
/// Measurement JSON with optional baseline evidence and explicit completeness and review flags.
/// Supported comparison deltas retain metric names and numeric values. This function performs
/// no I/O, secret resolution, or target requests.
///
/// # Examples
/// ```
/// # fn example(report: &velo_contracts::reports::ComparisonReport) {
/// let selected = velo_contracts::analysis::comparison_evidence(report);
/// assert!(selected.get("candidate").is_some());
/// assert!(selected.get("configuration").is_none());
/// # }
/// ```
pub fn comparison_evidence(report: &ComparisonReport) -> Value {
    redact_evidence(&json!({
        "candidate": measurements(&report.candidate),
        "baseline": report.baseline.as_ref().map(measurements),
        "deltas": report.deltas,
        "comparison_limited": !report.limitations.is_empty(),
        "configuration_reviewed": report.configuration_reviewed,
        "configuration_differences": report.differences.len(),
    }))
}

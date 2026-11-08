use serde_json::{Value, json};
use velo_contracts::{HistoryEntry, reports::*};

fn entry(id: &str) -> HistoryEntry {
    let latency = json!({"samples":"100", "out_of_range":"0", "median_us":1000, "p95_us":2000, "p99_us":3000});
    let metrics = json!({
        "counter_overflow":false, "aggregation_interference":false,
        "response_latency":latency, "transport_latency":{"out_of_range":"0"}, "timeout_latency":{"out_of_range":"0"},
        "http_failure_rate":0.01, "assertion_failure_rate":0.02, "request_rate":100.0,
        "counters":{"started":"18446744073709551615", "responses":"18446744073709551615"},
        "diagnostics":{"assertions":[{"id":"status", "passed":"98", "failed":"2", "unavailable":"0"}], "samples":[], "groups":[]}
    });
    HistoryEntry {
        id: id.into(),
        method: "GET".into(),
        created_at: "1700000000".into(),
        interrupted: false,
        incomplete: false,
        stride: 4,
        points: vec![],
        metadata: Default::default(),
        definition_identity: Some("safe-v1:fixture".into()),
        configuration: json!({"timeout_ms":30000}),
        snapshot: json!({
            "state":"completed", "confirmation":{"origin":"https://example.com", "workload":{"kind":"constant", "vus":2, "duration_ms":1000}, "duration_ms":1000, "warmup_ms":0, "dataset":null},
            "measurement":{"complete":true, "warmup_ms":0, "evaluated_us":"1000000"},
            "metrics":metrics, "evaluated":metrics, "arrival":{"capacity_drops":"0", "missed_deadlines":"0"},
            "thresholds":[{"rule":{"metric":"response_p95_ms", "operator":"less_than", "limit":1}, "observed":2.0, "status":"failed"}],
            "scenarios":null, "dataset":null, "telemetry":{"status":"unavailable"}
        }),
    }
}

fn settings(metric: RegressionMetric, limit: f64, percentage: bool) -> ComparisonSettings {
    ComparisonSettings {
        baseline_id: "baseline".into(),
        configuration_reviewed: true,
        budgets: vec![RegressionBudget {
            metric,
            limit,
            percentage,
        }],
    }
}

#[test]
fn unrounded_deltas_and_direction_are_independent_of_absolute_thresholds() {
    let baseline = entry("baseline");
    let mut candidate = entry("candidate");
    candidate.snapshot["evaluated"]["response_latency"]["p95_us"] = json!(2251);
    candidate.snapshot["evaluated"]["request_rate"] = json!(89.5);
    let report = compare(
        Some(baseline.clone()),
        candidate.clone(),
        &settings(RegressionMetric::ResponseP95Ms, 12.5, true),
    );
    assert_eq!(report.verdict, RegressionVerdict::Fail);
    let delta = report
        .deltas
        .iter()
        .find(|delta| delta.metric == RegressionMetric::ResponseP95Ms)
        .unwrap();
    assert!((delta.absolute.unwrap() - 0.251).abs() < 1e-12);
    assert!((delta.percentage.unwrap() - 12.55).abs() < 1e-12);
    let report = compare(
        Some(baseline.clone()),
        candidate.clone(),
        &settings(RegressionMetric::RequestRate, 10.5, true),
    );
    assert_eq!(report.verdict, RegressionVerdict::Pass);
    assert_eq!(report.regressions[0].worsening, Some(10.5));
    assert_eq!(
        report.candidate.snapshot["thresholds"][0]["status"],
        "failed"
    );
    let report = compare(
        Some(baseline),
        candidate,
        &settings(RegressionMetric::RequestRate, 10.49, true),
    );
    assert_eq!(report.verdict, RegressionVerdict::Fail);
}

#[test]
fn zero_and_missing_baselines_have_no_relative_percentage() {
    let mut baseline = entry("baseline");
    baseline.snapshot["evaluated"]["http_failure_rate"] = json!(0.0);
    let report = compare(
        Some(baseline.clone()),
        entry("candidate"),
        &settings(RegressionMetric::HttpFailurePercent, 10.0, true),
    );
    assert_eq!(report.verdict, RegressionVerdict::Unevaluable);
    let delta = report
        .deltas
        .iter()
        .find(|delta| delta.metric == RegressionMetric::HttpFailurePercent)
        .unwrap();
    assert_eq!(delta.percentage, None);
    assert_eq!(delta.absolute, Some(1.0));
    let report = compare(
        Some(baseline),
        entry("candidate"),
        &settings(RegressionMetric::HttpFailurePercent, 1.0, false),
    );
    assert_eq!(report.verdict, RegressionVerdict::Pass);
    let report = compare(
        None,
        entry("candidate"),
        &settings(RegressionMetric::RequestRate, 100.0, false),
    );
    assert_eq!(report.verdict, RegressionVerdict::Unevaluable);
    assert!(
        report
            .deltas
            .iter()
            .all(|delta| delta.percentage.is_none() && delta.absolute.is_none())
    );
    assert_eq!(report.baseline_id, "baseline");
}

#[test]
fn every_partial_or_integrity_limited_state_prevents_a_clean_pass() {
    let baseline = entry("baseline");
    for state in [
        "cancelled",
        "aborted",
        "failed",
        "running",
        "stopping",
        "starting",
    ] {
        let mut candidate = entry("candidate");
        candidate.snapshot["state"] = json!(state);
        assert_eq!(
            compare(
                Some(baseline.clone()),
                candidate,
                &settings(RegressionMetric::RequestRate, 0.0, true)
            )
            .verdict,
            RegressionVerdict::Unevaluable,
            "{state}"
        );
    }
    for pointer in [
        "/measurement/complete",
        "/evaluated/counter_overflow",
        "/metrics/aggregation_interference",
        "/evaluated/response_latency/out_of_range",
        "/arrival/missed_deadlines",
        "/arrival/capacity_drops",
    ] {
        let mut candidate = entry("candidate");
        let value = candidate.snapshot.pointer_mut(pointer).unwrap();
        *value = if pointer.ends_with("complete") {
            json!(false)
        } else if value.is_string() {
            json!("1")
        } else {
            json!(true)
        };
        assert_eq!(
            compare(
                Some(baseline.clone()),
                candidate,
                &settings(RegressionMetric::RequestRate, 100.0, false)
            )
            .verdict,
            RegressionVerdict::Unevaluable,
            "{pointer}"
        );
    }
    for (interrupted, incomplete) in [(true, false), (false, true)] {
        let mut candidate = entry("candidate");
        candidate.interrupted = interrupted;
        candidate.incomplete = incomplete;
        assert_eq!(
            compare(
                Some(baseline.clone()),
                candidate,
                &settings(RegressionMetric::RequestRate, 100.0, false)
            )
            .verdict,
            RegressionVerdict::Unevaluable
        );
    }
}

#[test]
fn compatibility_is_not_overridden_by_configuration_acknowledgement() {
    for pointer in ["/confirmation/origin", "/confirmation/workload/kind"] {
        let mut candidate = entry("candidate");
        *candidate.snapshot.pointer_mut(pointer).unwrap() = json!("different");
        let report = compare(
            Some(entry("baseline")),
            candidate,
            &settings(RegressionMetric::RequestRate, 100.0, false),
        );
        assert_eq!(report.verdict, RegressionVerdict::Unevaluable);
        assert!(
            report
                .limitations
                .iter()
                .any(|text| text.contains("blocked"))
        );
    }
    let mut candidate = entry("candidate");
    candidate.method = "POST".into();
    assert_eq!(
        compare(
            Some(entry("baseline")),
            candidate,
            &settings(RegressionMetric::RequestRate, 100.0, false)
        )
        .verdict,
        RegressionVerdict::Unevaluable
    );
    assert_eq!(
        compare(
            Some(entry("baseline")),
            entry("baseline"),
            &settings(RegressionMetric::RequestRate, 100.0, false)
        )
        .verdict,
        RegressionVerdict::Unevaluable
    );
}

#[test]
fn differences_and_redacted_context_require_review_and_missing_identity_cannot_pass() {
    let mut rules = settings(RegressionMetric::RequestRate, 0.0, false);
    rules.configuration_reviewed = false;
    let mut candidate = entry("candidate");
    candidate.snapshot["confirmation"]["duration_ms"] = json!(2000);
    candidate.configuration["timeout_ms"] = json!(2000);
    let report = compare(Some(entry("baseline")), candidate.clone(), &rules);
    assert_eq!(report.verdict, RegressionVerdict::Unevaluable);
    assert!(
        report
            .differences
            .iter()
            .any(|difference| difference.field == "duration_ms")
    );
    assert!(
        report
            .differences
            .iter()
            .any(|difference| difference.field == "execution_settings")
    );
    rules.configuration_reviewed = true;
    assert_eq!(
        compare(Some(entry("baseline")), candidate.clone(), &rules).verdict,
        RegressionVerdict::Pass
    );
    candidate.definition_identity = None;
    assert_eq!(
        compare(Some(entry("baseline")), candidate, &rules).verdict,
        RegressionVerdict::Unevaluable
    );
}

#[test]
fn missing_samples_and_absent_optional_assertions_are_unevaluable() {
    for pointer in [
        "/evaluated/assertion_failure_rate",
        "/evaluated/response_latency/p95_us",
        "/measurement/evaluated_us",
    ] {
        let mut candidate = entry("candidate");
        *candidate.snapshot.pointer_mut(pointer).unwrap() = Value::Null;
        let metric = if pointer.contains("assertion") {
            RegressionMetric::AssertionFailurePercent
        } else {
            RegressionMetric::ResponseP95Ms
        };
        assert_eq!(
            compare(
                Some(entry("baseline")),
                candidate,
                &settings(metric, 100.0, false)
            )
            .verdict,
            RegressionVerdict::Unevaluable
        );
    }
    let mut candidate = entry("candidate");
    candidate.snapshot["evaluated"]["response_latency"]["samples"] = json!("0");
    assert_eq!(
        compare(
            Some(entry("baseline")),
            candidate,
            &settings(RegressionMetric::ResponseP95Ms, 100.0, false)
        )
        .verdict,
        RegressionVerdict::Unevaluable
    );
}

#[test]
fn invalid_budget_and_metadata_inputs_are_rejected_and_never_pass() {
    for limit in [f64::NAN, f64::INFINITY, -1.0, 1e13] {
        let rules = settings(RegressionMetric::RequestRate, limit, false);
        assert_eq!(
            validate_settings(&rules).unwrap_err().code,
            "INVALID_REPORT_SETTINGS"
        );
        assert_eq!(
            compare(Some(entry("baseline")), entry("candidate"), &rules).verdict,
            RegressionVerdict::Unevaluable
        );
    }
    let mut metadata = RunMetadata {
        git_sha: "not-a-sha".into(),
        ..Default::default()
    };
    assert!(validate_metadata(&metadata).is_err());
    metadata.git_sha = "a1b2c3d".into();
    metadata.notes = "é".repeat(2001);
    assert!(validate_metadata(&metadata).is_err());
    metadata.notes.clear();
    assert!(validate_metadata(&metadata).is_ok());
    assert!(serde_json::from_value::<ComparisonSettings>(json!({"baseline_id":"baseline", "budgets":[], "configuration_reviewed":true, "extra":"ignored?"})).is_err());
}

use serde_json::{Value, json};
use velo_contracts::{
    HistoryEntry,
    analysis::{comparison_evidence, redact_evidence},
    reports::{
        ComparisonReport, ConfigurationDifference, MetricDelta, RegressionMetric,
        RegressionVerdict, RunMetadata,
    },
};

fn entry() -> HistoryEntry {
    HistoryEntry {
        method: "GET".into(),
        id: "PRIVATE_ID".into(),
        created_at: "0".into(),
        interrupted: true,
        incomplete: true,
        stride: 2,
        metadata: RunMetadata {
            notes: "PRIVATE_NOTES".into(),
            ..Default::default()
        },
        definition_identity: Some("PRIVATE_IDENTITY".into()),
        configuration: json!({"headers": [{"name":"PRIVATE_HEADER", "value":"PRIVATE_VALUE"}]}),
        snapshot: json!({
            "metrics": {
                "request_rate": 12.5,
                "http_failure_rate": null,
                "counters": {"responses": "18446744073709551615"},
                "diagnostics": {
                    "assertions": [{"id":"PRIVATE_ASSERTION", "passed":"3"}],
                    "samples": [{"body":"PRIVATE_BODY", "status":503}],
                },
            },
            "evaluated": {"request_rate": 10.0, "counter_overflow":true},
            "measurement": {"complete":false, "evaluated_us":"123456789"},
            "arrival": {"capacity_drops":"4"},
            "telemetry": {"process_cpu_percent":null},
            "confirmation": {"origin":"https://PRIVATE.example"},
        }),
        points: vec![json!({"PRIVATE_TIMELINE":true})],
    }
}

#[test]
fn comparison_selection_keeps_measurement_integrity_without_user_content() {
    let report = ComparisonReport {
        format_version: 1,
        baseline_id: "PRIVATE_BASELINE".into(),
        baseline: Some(entry()),
        candidate: entry(),
        configuration_reviewed: false,
        differences: vec![ConfigurationDifference {
            field: "PRIVATE_DIFFERENCE".into(),
            baseline: json!("PRIVATE_BASELINE_VALUE"),
            candidate: json!("PRIVATE_CANDIDATE_VALUE"),
        }],
        limitations: vec!["PRIVATE_LIMITATION".into()],
        deltas: vec![MetricDelta {
            metric: RegressionMetric::ResponseP95Ms,
            baseline: Some(2.0),
            candidate: Some(3.0),
            absolute: Some(1.0),
            percentage: Some(50.0),
        }],
        regressions: vec![],
        verdict: RegressionVerdict::Unevaluable,
    };
    let selected = comparison_evidence(&report);
    assert!(!selected.to_string().contains("PRIVATE"));
    assert_eq!(selected["candidate"]["incomplete"], true);
    assert_eq!(selected["candidate"]["interrupted"], true);
    assert_eq!(selected["candidate"]["stride"], 2);
    assert_eq!(selected["candidate"]["measurement"]["complete"], false);
    assert_eq!(
        selected["candidate"]["metrics"]["counters"]["responses"],
        u64::MAX
    );
    assert_eq!(selected["candidate"]["arrival"]["capacity_drops"], 4);
    assert_eq!(selected["comparison_limited"], true);
    assert_eq!(selected["configuration_differences"], 1);
    assert_eq!(selected["configuration_reviewed"], false);
    assert_eq!(selected["deltas"][0]["metric"], "response_p95_ms");
    assert_eq!(selected["deltas"][0]["percentage"], 50.0);
    assert!(selected["candidate"]["metrics"]["http_failure_rate"].is_null());
    assert!(selected["candidate"]["telemetry"]["process_cpu_percent"].is_null());
}

#[test]
fn unrecognized_keys_and_scalar_text_do_not_survive_selection() {
    let selected = redact_evidence(&json!({
        "PRIVATE_ROOT": 123,
        "metrics": {
            "PRIVATE_NESTED": true,
            "request_rate": "PRIVATE_TEXT",
            "counters": {"responses":"18446744073709551616", "timeouts":"-1"},
        },
        "deltas": [
            {"metric":"PRIVATE_METRIC", "baseline":null},
            {"metric":"request_rate", "candidate":0},
        ],
    }));
    assert!(!selected.to_string().contains("PRIVATE"));
    assert!(selected["metrics"]["request_rate"].is_null());
    assert!(selected["metrics"]["counters"]["responses"].is_null());
    assert!(selected["metrics"]["counters"]["timeouts"].is_null());
    assert!(selected["deltas"][0]["metric"].is_null());
    assert_eq!(selected["deltas"][1]["metric"], "request_rate");
    assert_eq!(selected["deltas"][1]["candidate"], 0);
}

#[test]
fn arrays_and_nested_values_are_bounded_without_losing_first_entries() {
    let selected = redact_evidence(&json!({"samples":vec![1;129]}));
    assert_eq!(selected["samples"].as_array().unwrap().len(), 128);
    assert_eq!(selected["samples"][0], 1);
    let mut nested = json!(true);
    for _ in 0..14 {
        nested = Value::Array(vec![nested]);
    }
    let selected = redact_evidence(&nested);
    let mut current = &selected;
    for _ in 0..13 {
        current = &current[0];
    }
    assert!(current.is_null());
}

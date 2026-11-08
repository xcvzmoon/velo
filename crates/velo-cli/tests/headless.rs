use std::{
    process::{Command, Output},
    sync::{
        Arc,
        atomic::{AtomicU16, AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use serde_json::{Value, json};
use velo_engine::{Assertion, AssertionRule, RunConfig, TestDefinition, Workload};

struct Target {
    origin: String,
    requests: Arc<AtomicUsize>,
    status: Arc<AtomicU16>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Target {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn target() -> Target {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(AtomicUsize::new(0));
    let status = Arc::new(AtomicU16::new(200));
    let count = requests.clone();
    let response_status = status.clone();
    let router = Router::new().route(
        "/{*path}",
        get(move |headers: HeaderMap| {
            count.fetch_add(1, Ordering::Relaxed);
            let status = response_status.load(Ordering::Relaxed);
            async move {
                (
                    StatusCode::from_u16(status).unwrap(),
                    headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("ok")
                        .to_owned(),
                )
            }
        }),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Target {
        origin,
        requests,
        status,
        server,
    }
}

fn definition(origin: &str) -> TestDefinition {
    TestDefinition {
        config: RunConfig {
            url: format!("{origin}/private"),
            workload: Workload::Constant {
                vus: 1,
                duration_ms: 80,
            },
            grace_ms: 30,
            pacing_ms: 5,
            ..RunConfig::default()
        },
        ..TestDefinition::default()
    }
}

fn command(directory: &tempfile::TempDir, definition: &TestDefinition) -> Command {
    let path = directory.path().join("velo.test.json");
    std::fs::write(&path, serde_json::to_vec(definition).unwrap()).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_velo-cli"));
    command.arg(path).current_dir(directory.path());
    command
}

async fn invoke(mut command: Command) -> Output {
    tokio::task::spawn_blocking(move || command.output().unwrap())
        .await
        .unwrap()
}

fn code(output: &Output) -> i32 {
    output.status.code().unwrap()
}

fn artifact(directory: &tempfile::TempDir) -> Value {
    serde_json::from_slice(&std::fs::read(directory.path().join("report.json")).unwrap()).unwrap()
}

#[tokio::test]
async fn preparation_and_authorization_rejections_send_no_traffic() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let definition = definition(&target.origin);
    let mut prepare = command(&directory, &definition);
    prepare.arg("--prepare");
    assert_eq!(code(&invoke(prepare).await), 0);
    let unauthorized = command(&directory, &definition);
    assert_eq!(code(&invoke(unauthorized).await), 2);
    let mut wrong_target = command(&directory, &definition);
    wrong_target.args(["--authorize", "http://127.0.0.1:1"]);
    assert_eq!(code(&invoke(wrong_target).await), 2);
    assert_eq!(target.requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn strict_inputs_missing_secrets_and_environments_fail_before_execution() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let mut definition = definition(&target.origin);
    definition.config.auth = velo_engine::RequestAuth::Bearer {
        token: "{{secret:token}}".into(),
    };
    let mut missing = command(&directory, &definition);
    missing.arg("--prepare");
    let output = invoke(missing).await;
    assert_eq!(code(&output), 2);
    assert!(String::from_utf8_lossy(&output.stderr).contains("SECRET_UNAVAILABLE"));
    let mut missing_environment = command(&directory, &definition);
    missing_environment.args(["--prepare", "--environment", "missing"]);
    assert_eq!(code(&invoke(missing_environment).await), 2);
    let mut invalid = command(&directory, &definition);
    let mut value = serde_json::to_value(&definition).unwrap();
    value["config"]["unsupported_capability"] = json!(true);
    std::fs::write(directory.path().join("velo.test.json"), value.to_string()).unwrap();
    invalid.arg("--prepare");
    assert_eq!(code(&invoke(invalid).await), 2);
    assert_eq!(target.requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn exported_definition_runs_with_environment_bindings_and_safe_shared_artifacts() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let mut definition = definition("https://unused.example");
    definition.config.url = "{{base}}/private?key={{token}}".into();
    definition.config.auth = velo_engine::RequestAuth::Bearer {
        token: "{{token}}".into(),
    };
    definition
        .variables
        .insert("token".into(), "{{secret:ci-token}}".into());
    definition.environments.insert(
        "ci".into(),
        [("base".into(), "https://unused.example".into())].into(),
    );
    definition.config.assertions = vec![Assertion {
        id: "echo".into(),
        rule: AssertionRule::TextEquals {
            value: "Bearer PRIVATE_CI_TOKEN".into(),
        },
    }];
    let mut run = command(&directory, &definition);
    run.args([
        "--authorize",
        &target.origin,
        "--environment",
        "ci",
        "--variable",
        "base=VELO_TEST_ORIGIN",
        "--secret",
        "ci-token=VELO_TEST_TOKEN",
        "--json",
        "report.json",
        "--junit",
        "report.xml",
        "--html",
        "report.html",
    ])
    .env("VELO_TEST_ORIGIN", &target.origin)
    .env("VELO_TEST_TOKEN", "PRIVATE_CI_TOKEN");
    let output = invoke(run).await;
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(target.requests.load(Ordering::Relaxed) > 0);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PRIVATE_CI_TOKEN"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_CI_TOKEN"));
    for path in ["report.json", "report.xml", "report.html"] {
        let source = std::fs::read_to_string(directory.path().join(path)).unwrap();
        assert!(!source.contains("PRIVATE_CI_TOKEN"));
        assert!(!source.contains("/private"));
        assert!(source.contains(&target.origin));
    }
    let report = artifact(&directory);
    assert_eq!(report["format_version"], 1);
    assert_eq!(report["candidate"]["snapshot"]["state"], "completed");
    assert!(report["candidate"]["snapshot"]["metrics"]["counters"]["responses"].is_string());
    assert!(report["candidate"]["configuration"].get("url").is_none());
}

#[tokio::test]
async fn failed_assertions_and_thresholds_preserve_reports_and_fail_ci() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let mut definition = definition(&target.origin);
    definition.config.assertions = vec![Assertion {
        id: "status".into(),
        rule: AssertionRule::Status { allowed: vec![201] },
    }];
    definition.config.thresholds = serde_json::from_value(
        json!([{"metric":"request_rate","operator":"greater_than","limit":1e100}]),
    )
    .unwrap();
    let mut run = command(&directory, &definition);
    run.args([
        "--authorize",
        &target.origin,
        "--json",
        "report.json",
        "--junit",
        "report.xml",
    ]);
    assert_eq!(code(&invoke(run).await), 1);
    let report = artifact(&directory);
    assert_eq!(
        report["candidate"]["snapshot"]["thresholds"][0]["status"],
        "failed"
    );
    let junit = std::fs::read_to_string(directory.path().join("report.xml")).unwrap();
    assert!(junit.contains("<failure"));
    assert!(junit.contains("run outcome"));
}

#[tokio::test]
async fn regression_budgets_use_shared_baseline_rules() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let definition = definition(&target.origin);
    let mut first = command(&directory, &definition);
    first.args(["--authorize", &target.origin, "--json", "report.json"]);
    assert_eq!(code(&invoke(first).await), 0);
    let baseline = artifact(&directory);
    std::fs::rename(
        directory.path().join("report.json"),
        directory.path().join("baseline.json"),
    )
    .unwrap();
    std::fs::write(directory.path().join("comparison.json"), json!({"baseline_id":baseline["candidate"]["id"], "configuration_reviewed":true, "budgets":[{"metric":"http_failure_percent","limit":0,"percentage":false}]}).to_string()).unwrap();
    target.status.store(500, Ordering::Relaxed);
    let mut second = command(&directory, &definition);
    second.args([
        "--authorize",
        &target.origin,
        "--baseline",
        "baseline.json",
        "--comparison",
        "comparison.json",
        "--json",
        "report.json",
    ]);
    assert_eq!(code(&invoke(second).await), 1);
    let report = artifact(&directory);
    assert_eq!(report["verdict"], "fail");
    assert_eq!(report["regressions"][0]["worsening"], 100.0);
    assert_eq!(report["baseline"]["id"], baseline["candidate"]["id"]);
}

#[tokio::test]
async fn datasets_require_explicit_rows_and_use_native_allocation() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let mut definition = definition(&target.origin);
    definition.config.url = format!("{}/{{{{item}}}}", target.origin);
    definition.dataset = Some(serde_json::from_value(json!({"lifetime":"iteration","allocation":"sequential","exhaustion":"stop","seed":42,"source":{"kind":"json","mapping":{"item":"id"}}})).unwrap());
    let mut missing = command(&directory, &definition);
    missing.arg("--prepare");
    assert_eq!(code(&invoke(missing).await), 2);
    std::fs::write(
        directory.path().join("rows.json"),
        "[{\"item\":\"first\"},{\"item\":\"second\"}]",
    )
    .unwrap();
    let mut run = command(&directory, &definition);
    run.args([
        "--authorize",
        &target.origin,
        "--data-rows",
        "rows.json",
        "--json",
        "report.json",
    ]);
    assert_eq!(code(&invoke(run).await), 1);
    assert_eq!(target.requests.load(Ordering::Relaxed), 2);
    let report = artifact(&directory);
    assert_eq!(
        report["candidate"]["snapshot"]["dataset"]["exhausted"],
        true
    );
    assert!(!report.to_string().contains("first"));
}

#[tokio::test]
async fn invalid_destinations_cannot_overwrite_inputs_and_export_errors_stay_nonzero() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let definition = definition(&target.origin);
    let mut collision = command(&directory, &definition);
    collision.args(["--authorize", &target.origin, "--json", "velo.test.json"]);
    assert_eq!(code(&invoke(collision).await), 2);
    assert_eq!(target.requests.load(Ordering::Relaxed), 0);
    std::fs::create_dir(directory.path().join("blocked.json")).unwrap();
    let mut run = command(&directory, &definition);
    run.args([
        "--authorize",
        &target.origin,
        "--json",
        "blocked.json",
        "--junit",
        "report.xml",
    ]);
    let output = invoke(run).await;
    assert_eq!(code(&output), 3);
    assert!(String::from_utf8_lossy(&output.stderr).contains("REPORT_WRITE_FAILED"));
    assert!(directory.path().join("report.xml").exists());
}

#[tokio::test]
async fn cli_parser_errors_do_not_echo_arbitrary_input() {
    let directory = tempfile::tempdir().unwrap();
    let mut invalid = command(&directory, &definition("http://127.0.0.1:1"));
    invalid.arg("--PRIVATE_COMMAND_VALUE");
    let output = invoke(invalid).await;
    assert_eq!(code(&output), 2);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_COMMAND_VALUE"));
}

#[cfg(unix)]
#[tokio::test]
async fn termination_drains_and_exports_partial_evidence() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let mut definition = definition(&target.origin);
    definition.config.workload = Workload::Constant {
        vus: 2,
        duration_ms: 60_000,
    };
    let mut run = command(&directory, &definition);
    run.args(["--authorize", &target.origin, "--json", "report.json"]);
    let child = run.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while target.requests.load(Ordering::Relaxed) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let output = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::task::spawn_blocking(move || child.wait_with_output().unwrap()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(code(&output), 130);
    let report = artifact(&directory);
    assert_eq!(report["candidate"]["snapshot"]["state"], "cancelled");
    assert_eq!(report["candidate"]["snapshot"]["active_vus"], 0);
    assert_eq!(report["candidate"]["interrupted"], true);
    assert_eq!(
        report["candidate"]["snapshot"]["measurement"]["complete"],
        false
    );
}

#[tokio::test]
async fn secrets_can_arrive_over_stdin_without_a_credential_store_or_persistence() {
    use std::{io::Write, process::Stdio};
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let mut definition = definition(&target.origin);
    definition.config.auth = velo_engine::RequestAuth::Bearer {
        token: "{{secret:saved.portable.field}}".into(),
    };
    let mut run = command(&directory, &definition);
    run.args([
        "--authorize",
        &target.origin,
        "--secrets-stdin",
        "--json",
        "report.json",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let output = tokio::task::spawn_blocking(move || {
        let mut child = run.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"saved.portable.field\":\"PRIVATE_STDIN_VALUE\"}")
            .unwrap();
        child.wait_with_output().unwrap()
    })
    .await
    .unwrap();
    assert_eq!(code(&output), 0);
    assert!(
        !artifact(&directory)
            .to_string()
            .contains("PRIVATE_STDIN_VALUE")
    );
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[tokio::test]
async fn all_transport_failures_have_an_execution_status_and_a_report() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let directory = tempfile::tempdir().unwrap();
    let mut run = command(&directory, &definition(&origin));
    run.args(["--authorize", &origin, "--json", "report.json"]);
    assert_eq!(code(&invoke(run).await), 3);
    assert_eq!(
        artifact(&directory)["candidate"]["snapshot"]["metrics"]["counters"]["responses"],
        "0"
    );
}

#[tokio::test]
async fn scenario_setup_load_and_cleanup_use_the_shared_engine() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let mut definition = definition(&target.origin);
    let step = json!({"id":"request", "request":{"url":format!("{}/action", target.origin), "method":"GET", "headers":[], "query":[], "auth":{"kind":"none"}, "body":{"kind":"none"}}, "on_failure":"stop"});
    definition.config.scenarios = Some(serde_json::from_value(json!({"setup":[step.clone()], "cleanup":[step.clone()], "scenarios":[{"id":"journey","weight":1,"steps":[step]}]})).unwrap());
    let mut run = command(&directory, &definition);
    run.args(["--authorize", &target.origin, "--json", "report.json"]);
    let output = invoke(run).await;
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = artifact(&directory);
    let scenario = &report["candidate"]["snapshot"]["scenarios"];
    assert_eq!(scenario["setup"], "succeeded");
    assert_eq!(scenario["cleanup"], "succeeded");
    assert!(
        scenario["iterations"][0]["completed"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            > 0
    );
    assert!(!report.to_string().contains("/action"));
}

#[tokio::test]
async fn redaction_preserves_schema_keys_enums_and_decimal_counters() {
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let mut definition = definition(&target.origin);
    definition.config.auth = velo_engine::RequestAuth::Bearer {
        token: "{{secret:token}}".into(),
    };
    definition.config.assertions = vec![Assertion {
        id: "id".into(),
        rule: AssertionRule::Status { allowed: vec![200] },
    }];
    let mut run = command(&directory, &definition);
    run.args([
        "--authorize",
        &target.origin,
        "--secret",
        "token=VELO_TEST_TOKEN",
        "--json",
        "report.json",
    ])
    .env("VELO_TEST_TOKEN", "id");
    assert_eq!(code(&invoke(run).await), 0);
    let report = artifact(&directory);
    let snapshot = &report["candidate"]["snapshot"];
    assert_eq!(snapshot["state"], "completed");
    assert_eq!(
        snapshot["evaluated"]["diagnostics"]["assertions"][0]["id"],
        "[redacted]"
    );
    assert!(
        snapshot["metrics"]["counters"]["responses"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            > 0
    );
}

#[cfg(unix)]
#[tokio::test]
async fn interruption_during_stdin_preparation_is_bounded_and_sends_no_traffic() {
    use std::process::Stdio;
    let target = target().await;
    let directory = tempfile::tempdir().unwrap();
    let mut run = command(&directory, &definition(&target.origin));
    run.args([
        "--secrets-stdin",
        "--authorize",
        &target.origin,
        "--json",
        "report.json",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = run.spawn().unwrap();
    let stdin = child.stdin.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (_reader, message) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            use std::io::BufRead;
            let mut reader = std::io::BufReader::new(stderr);
            let mut message = String::new();
            reader.read_line(&mut message).unwrap();
            (reader, message)
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(message.trim(), "Reading secret input from stdin.");
    assert!(
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let output = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::task::spawn_blocking(move || child.wait_with_output().unwrap()),
    )
    .await
    .unwrap()
    .unwrap();
    drop(stdin);
    assert_eq!(code(&output), 130);
    assert_eq!(target.requests.load(Ordering::Relaxed), 0);
    assert!(!directory.path().join("report.json").exists());
}

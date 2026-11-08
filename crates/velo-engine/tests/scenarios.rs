use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::StatusCode,
    response::Response,
    routing::any,
};
use tokio::{
    net::TcpListener,
    sync::Mutex,
    task::JoinHandle,
    time::{Instant, sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use velo_engine::{
    DataAllocation, DataDefinition, DataExhaustion, DataLifetime, DataSource, Extraction,
    ExtractionSource, NameValue, RequestAuth, RequestBody, RequestMethod, RunConfig, RunManager,
    RunSnapshot, RunState, Scenario, ScenarioPlan, ScenarioRequest, ScenarioStep, StepFailure,
    TestDefinition, Workload,
};

#[derive(Clone, Debug)]
struct Received {
    uri: String,
    authorization: Option<String>,
    cookie: Option<String>,
    extracted_header: Option<String>,
    extracted_cookie: Option<String>,
    body: Vec<u8>,
}

#[derive(Clone, Default)]
struct Target {
    received: Arc<Mutex<Vec<Received>>>,
    users: Arc<AtomicUsize>,
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}

struct Server {
    url: String,
    target: Target,
    stop: CancellationToken,
    task: JoinHandle<()>,
}

impl Server {
    async fn start() -> Self {
        let target = Target::default();
        let app = Router::new()
            .fallback(any(handle))
            .with_state(target.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stop = CancellationToken::new();
        let shutdown = stop.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
                .unwrap();
        });
        Self {
            url,
            target,
            stop,
            task,
        }
    }

    async fn close(mut self) {
        self.stop.cancel();
        timeout(Duration::from_secs(3), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

async fn handle(State(target): State<Target>, request: Request) -> Response {
    let active = target.active.fetch_add(1, Ordering::SeqCst) + 1;
    target.peak.fetch_max(active, Ordering::SeqCst);
    let (parts, body) = request.into_parts();
    let path = parts.uri.path().to_owned();
    let header = |name: &str| {
        parts
            .headers
            .get(name)
            .map(|value| value.to_str().unwrap().to_owned())
    };
    let received = Received {
        uri: parts.uri.to_string(),
        authorization: header("authorization"),
        cookie: header("cookie"),
        extracted_header: header("x-extracted-header"),
        extracted_cookie: header("x-extracted-cookie"),
        body: to_bytes(body, 1024 * 1024).await.unwrap().to_vec(),
    };
    target.received.lock().await.push(received);
    let response = match path.as_str() {
        "/login" => {
            let id = target.users.fetch_add(1, Ordering::SeqCst) + 1;
            let token = format!("user-{id}");
            Response::builder()
                .header("content-type", "application/json")
                .header("x-user", &token)
                .header(
                    "set-cookie",
                    format!("sid={token}; Path=/; HttpOnly; SameSite=Lax"),
                )
                .body(Body::from(format!(r#"{{"token":"{token}"}}"#)))
                .unwrap()
        }
        "/create" => Response::new(Body::from(
            r#"{"order":"order/with?reserved=1&quote=\"","text":"quoted \"value\"\\with {{session:other}}"}"#,
        )),
        "/invalid-json" => Response::new(Body::from("not JSON")),
        "/oversized-json" => {
            Response::new(Body::from(format!(r#"{{"token":"{}"}}"#, "x".repeat(1024))))
        }
        "/failure" => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("failed"))
            .unwrap(),
        "/drain" => {
            sleep(Duration::from_millis(500)).await;
            Response::new(Body::from("{}"))
        }
        "/slow" => {
            sleep(Duration::from_millis(30)).await;
            Response::new(Body::from("{}"))
        }
        "/cleanup-slow" => {
            sleep(Duration::from_millis(300)).await;
            Response::new(Body::from("{}"))
        }
        "/cookie-set" => Response::builder()
            .header("set-cookie", "scoped=inside; Path=/inside; HttpOnly")
            .header("set-cookie", "secure=private; Path=/secure; Secure")
            .header(
                "set-cookie",
                "foreign=private; Domain=example.invalid; Path=/",
            )
            .body(Body::from("{}"))
            .unwrap(),
        "/cookie-redirect" => Response::builder()
            .status(StatusCode::FOUND)
            .header("location", "/inside/redirected")
            .header("set-cookie", "redirect=retained; Path=/inside")
            .body(Body::from("{}"))
            .unwrap(),
        "/cookie-overflow" => {
            let mut response = Response::builder();
            for index in 0..33 {
                response = response.header("set-cookie", format!("cookie{index}=value; Path=/"));
            }
            response.body(Body::from("{}")).unwrap()
        }
        "/setup" => Response::new(Body::from(r#"{"record":"prepared-record"}"#)),
        _ => Response::new(Body::from("{}")),
    };
    target.active.fetch_sub(1, Ordering::SeqCst);
    response
}

fn step(base: &str, id: &str, path: &str) -> ScenarioStep {
    ScenarioStep {
        id: id.into(),
        request: ScenarioRequest {
            url: format!("{base}{path}"),
            method: RequestMethod::GET,
            headers: Vec::new(),
            query: Vec::new(),
            body: RequestBody::None,
            auth: RequestAuth::None,
            assertions: Vec::new(),
        },
        extractions: Vec::new(),
        think_ms: 0,
        on_failure: StepFailure::Stop,
    }
}

fn extract(name: &str, source: ExtractionSource) -> Extraction {
    Extraction {
        name: name.into(),
        source,
        required: true,
        overwrite: true,
    }
}

fn json_extraction(name: &str, pointer: &str) -> Extraction {
    extract(
        name,
        ExtractionSource::Json {
            pointer: pointer.into(),
        },
    )
}

fn config(base: &str, steps: Vec<ScenarioStep>) -> RunConfig {
    RunConfig {
        url: format!("{base}/"),
        workload: Workload::Arrival {
            rate: 1,
            capacity: 1,
            duration_ms: 100,
        },
        grace_ms: 1000,
        scenarios: Some(ScenarioPlan {
            seed: 42,
            setup: Vec::new(),
            cleanup: Vec::new(),
            scenarios: vec![Scenario {
                id: "journey".into(),
                weight: 1,
                steps,
            }],
        }),
        ..RunConfig::default()
    }
}

async fn terminal(manager: &RunManager) -> RunSnapshot {
    timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = manager.current().await.unwrap();
            if matches!(
                snapshot.state,
                RunState::Completed | RunState::Cancelled | RunState::Failed
            ) {
                assert_eq!(snapshot.active_vus, 0);
                assert_eq!(snapshot.metrics.counters.in_flight, 0);
                return snapshot;
            }
            sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap()
}

async fn finish(config: RunConfig) -> RunSnapshot {
    let manager = RunManager::default();
    manager.start(config, true).await.unwrap();
    let snapshot = terminal(&manager).await;
    manager.shutdown().await;
    snapshot
}

fn report(snapshot: &RunSnapshot) -> serde_json::Value {
    serde_json::to_value(snapshot).unwrap()["scenarios"].clone()
}

#[tokio::test]
async fn login_create_fetch_binds_json_headers_cookies_and_encoded_urls() {
    let server = Server::start().await;
    let mut login = step(&server.url, "login", "/login");
    login.extractions = vec![
        json_extraction("token", "/token"),
        extract(
            "header",
            ExtractionSource::Header {
                name: "x-user".into(),
            },
        ),
        extract("cookie", ExtractionSource::Cookie { name: "sid".into() }),
    ];
    let mut create = step(&server.url, "create", "/create");
    create.request.method = RequestMethod::POST;
    create.request.auth = RequestAuth::Bearer {
        token: "{{session:token}}".into(),
    };
    create.request.headers = vec![
        NameValue {
            name: "x-extracted-header".into(),
            value: "{{session:header}}".into(),
        },
        NameValue {
            name: "x-extracted-cookie".into(),
            value: "{{session:cookie}}".into(),
        },
    ];
    create.request.body = RequestBody::Json(r#"{"token":"{{session:token}}"}"#.into());
    create.extractions = vec![
        json_extraction("order", "/order"),
        json_extraction("text", "/text"),
    ];
    let mut fetch = step(&server.url, "fetch", "/fetch/{{session:order}}");
    fetch.request.method = RequestMethod::POST;
    fetch.request.auth = RequestAuth::Bearer {
        token: "{{session:token}}".into(),
    };
    fetch.request.query = vec![NameValue {
        name: "order".into(),
        value: "{{session:order}}".into(),
    }];
    fetch.request.body = RequestBody::Json(r#"{"text":"{{session:text}}"}"#.into());
    let snapshot = finish(config(&server.url, vec![login, create, fetch])).await;
    let received = server.target.received.lock().await.clone();
    assert_eq!(received.len(), 3);
    assert!(received[0].cookie.is_none());
    for request in &received[1..] {
        assert_eq!(request.authorization.as_deref(), Some("Bearer user-1"));
        assert_eq!(request.cookie.as_deref(), Some("sid=user-1"));
    }
    assert_eq!(received[1].extracted_header.as_deref(), Some("user-1"));
    assert_eq!(received[1].extracted_cookie.as_deref(), Some("user-1"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&received[1].body).unwrap()["token"],
        "user-1"
    );
    assert!(received[2].uri.contains("%2F"));
    assert!(!received[2].uri.contains("{{"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&received[2].body).unwrap()["text"],
        "quoted \"value\"\\with {{session:other}}"
    );
    assert_eq!(snapshot.metrics.counters.responses, 3);
    assert_eq!(snapshot.metrics.counters.completed_iterations, 1);
    assert!(snapshot.metrics.request_rate > snapshot.metrics.iteration_rate * 2.9);
    assert_eq!(report(&snapshot)["iterations"][0]["completed"], "1");
    server.close().await;
}

#[tokio::test]
async fn arrival_iterations_do_not_share_cookies_or_extracted_authentication() {
    let server = Server::start().await;
    let mut login = step(&server.url, "login", "/login");
    login.extractions.push(json_extraction("token", "/token"));
    let mut request = step(&server.url, "authenticated", "/slow");
    request.request.auth = RequestAuth::Bearer {
        token: "{{session:token}}".into(),
    };
    let mut input = config(&server.url, vec![login, request]);
    input.workload = Workload::Arrival {
        rate: 50,
        capacity: 3,
        duration_ms: 120,
    };
    let snapshot = finish(input).await;
    let received = server.target.received.lock().await.clone();
    let mut logins = 0;
    for request in &received {
        if request.uri == "/login" {
            logins += 1;
            assert!(
                request.cookie.is_none(),
                "arrival session inherited a cookie"
            );
        } else {
            let token = request
                .authorization
                .as_deref()
                .unwrap()
                .strip_prefix("Bearer ")
                .unwrap();
            assert_eq!(
                request.cookie.as_deref(),
                Some(format!("sid={token}").as_str())
            );
        }
    }
    assert!(logins >= 2);
    assert!(server.target.peak.load(Ordering::SeqCst) <= 3);
    assert!(snapshot.max_vus <= 3);
    assert_eq!(snapshot.metrics.counters.responses, logins * 2);
    assert_eq!(snapshot.metrics.counters.completed_iterations, logins);
    assert_eq!(
        snapshot.arrival.offered,
        snapshot.metrics.counters.completed_iterations
            + snapshot.arrival.capacity_drops
            + snapshot.arrival.missed_deadlines
    );
    server.close().await;
}

#[tokio::test]
async fn failed_extraction_and_unresolved_dependencies_skip_http_but_continue_independent_steps() {
    let server = Server::start().await;
    let mut missing = step(&server.url, "missing", "/empty");
    missing
        .extractions
        .push(json_extraction("required", "/absent"));
    missing.on_failure = StepFailure::Continue;
    let mut dependent = step(&server.url, "dependent", "/forbidden/{{session:required}}");
    dependent.on_failure = StepFailure::Continue;
    let independent = step(&server.url, "independent", "/independent");
    let snapshot = finish(config(&server.url, vec![missing, dependent, independent])).await;
    let received = server.target.received.lock().await.clone();
    assert_eq!(
        received
            .iter()
            .map(|request| request.uri.as_str())
            .collect::<Vec<_>>(),
        ["/empty", "/independent"]
    );
    let report = report(&snapshot);
    assert_eq!(report["iterations"][0]["failed"], "1");
    assert_eq!(report["steps"][0]["extraction_failures"], "1");
    assert_eq!(report["steps"][1]["binding_failures"], "1");
    assert_eq!(snapshot.metrics.counters.started, 2);
    server.close().await;
}

#[tokio::test]
async fn extraction_stop_policy_skips_remaining_steps_for_invalid_or_oversized_bodies() {
    let server = Server::start().await;
    for path in ["/invalid-json", "/oversized-json"] {
        let mut first = step(&server.url, "extract", path);
        first.extractions.push(json_extraction("token", "/token"));
        let second = step(&server.url, "must-not-run", "/forbidden");
        let mut input = config(&server.url, vec![first, second]);
        input.inspection.body_bytes = 64;
        let snapshot = finish(input).await;
        assert_eq!(snapshot.metrics.counters.responses, 1);
        let report = report(&snapshot);
        assert_eq!(report["iterations"][0]["failed"], "1");
        assert_eq!(report["steps"][0]["extraction_failures"], "1");
        assert!(!report["steps"][0]["last_error"].is_null());
    }
    assert!(
        server
            .target
            .received
            .lock()
            .await
            .iter()
            .all(|request| request.uri != "/forbidden")
    );
    server.close().await;
}

#[test]
fn session_values_cannot_change_authorized_scheme_or_authority() {
    for url in [
        "http://{{session:host}}/",
        "{{session:scheme}}://127.0.0.1/",
        "http://127.0.0.1:{{session:port}}/",
    ] {
        let mut request = step("http://127.0.0.1:8080", "unsafe", "/");
        request.request.url = url.into();
        assert!(RunManager::prepare(config("http://127.0.0.1:8080", vec![request])).is_err());
    }
    let external = step("http://127.0.0.1:8081", "external", "/");
    assert!(RunManager::prepare(config("http://127.0.0.1:8080", vec![external])).is_err());
}

#[tokio::test]
async fn setup_and_cleanup_are_separate_from_measured_load_and_use_setup_state() {
    let server = Server::start().await;
    let load = step(&server.url, "load", "/load/{{session:record}}");
    let mut setup = step(&server.url, "prepare", "/setup");
    setup.extractions.push(json_extraction("record", "/record"));
    let cleanup = step(&server.url, "remove", "/cleanup/{{session:record}}");
    let mut input = config(&server.url, vec![load]);
    let plan = input.scenarios.as_mut().unwrap();
    plan.setup.push(setup);
    plan.cleanup.push(cleanup);
    let snapshot = finish(input).await;
    let received = server.target.received.lock().await.clone();
    assert_eq!(
        received
            .iter()
            .map(|request| request.uri.as_str())
            .collect::<Vec<_>>(),
        [
            "/setup",
            "/load/prepared%2Drecord",
            "/cleanup/prepared%2Drecord"
        ]
    );
    assert_eq!(snapshot.metrics.counters.responses, 1);
    let report = report(&snapshot);
    assert_eq!(report["setup"], "succeeded");
    assert_eq!(report["cleanup"], "succeeded");
    server.close().await;
}

#[tokio::test]
async fn failed_setup_prevents_load_and_still_attempts_cleanup() {
    let server = Server::start().await;
    let mut input = config(&server.url, vec![step(&server.url, "load", "/forbidden")]);
    let plan = input.scenarios.as_mut().unwrap();
    plan.setup.push(step(&server.url, "prepare", "/failure"));
    plan.cleanup.push(step(&server.url, "remove", "/cleanup"));
    let snapshot = finish(input).await;
    assert_eq!(snapshot.state, RunState::Failed);
    assert_eq!(snapshot.metrics.counters.started, 0);
    assert_eq!(report(&snapshot)["setup"], "failed");
    assert_eq!(snapshot.arrival.offered, 0);
    assert_eq!(snapshot.arrival.missed_deadlines, 0);
    assert_eq!(snapshot.metrics.counters.completed_iterations, 0);
    assert_eq!(report(&snapshot)["iterations"][0]["started"], "0");
    assert_eq!(report(&snapshot)["cleanup"], "succeeded");
    let received = server.target.received.lock().await.clone();
    assert_eq!(
        received
            .iter()
            .map(|request| request.uri.as_str())
            .collect::<Vec<_>>(),
        ["/failure", "/cleanup"]
    );
    server.close().await;
}

#[tokio::test]
async fn stop_cancels_think_time_and_cleanup_failure_remains_visible() {
    let server = Server::start().await;
    let mut load = step(&server.url, "think", "/load");
    load.think_ms = 60_000;
    let mut input = config(&server.url, vec![load]);
    input.workload = Workload::Constant {
        vus: 1,
        duration_ms: 60_000,
    };
    input
        .scenarios
        .as_mut()
        .unwrap()
        .cleanup
        .push(step(&server.url, "remove", "/failure"));
    let manager = RunManager::default();
    let started = manager.start(input, true).await.unwrap();
    timeout(Duration::from_secs(3), async {
        while server.target.received.lock().await.is_empty() {
            sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    let stopping = Instant::now();
    manager.stop(&started.run_id).await.unwrap();
    let snapshot = terminal(&manager).await;
    assert!(stopping.elapsed() < Duration::from_secs(2));
    assert_eq!(report(&snapshot)["cleanup"], "failed");
    assert!(snapshot.failure_code.is_some());
    assert_eq!(snapshot.metrics.counters.responses, 1);
    assert!(snapshot.metrics.response_latency.max_us.unwrap() < 1_000_000);
    assert_eq!(server.target.received.lock().await.len(), 2);
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn virtual_users_retain_their_own_state_across_iterations() {
    let server = Server::start().await;
    let mut previous = step(&server.url, "previous", "/remember/{{session:token}}");
    previous.on_failure = StepFailure::Continue;
    previous.request.auth = RequestAuth::Bearer {
        token: "{{session:token}}".into(),
    };
    let mut login = step(&server.url, "login", "/login");
    login.extractions.push(json_extraction("token", "/token"));
    let mut input = config(&server.url, vec![previous, login]);
    input.workload = Workload::Constant {
        vus: 4,
        duration_ms: 100,
    };
    input.pacing_ms = 10;
    let snapshot = finish(input).await;
    let received = server.target.received.lock().await.clone();
    let first_logins = received
        .iter()
        .filter(|request| request.uri == "/login" && request.cookie.is_none())
        .count();
    assert_eq!(first_logins, 4);
    let mut repeated = 0;
    for request in &received {
        if request.uri.starts_with("/remember/") {
            repeated += 1;
            let token = request
                .authorization
                .as_deref()
                .unwrap()
                .strip_prefix("Bearer ")
                .unwrap();
            assert_eq!(
                request.cookie.as_deref(),
                Some(format!("sid={token}").as_str())
            );
            assert!(request.uri.ends_with(&token.replace('-', "%2D")));
        }
    }
    assert!(repeated >= 4);
    assert!(snapshot.max_vus <= 4);
    server.close().await;
}

#[tokio::test]
async fn missing_optional_extraction_removes_previous_value_before_dependent_binding() {
    let server = Server::start().await;
    let mut setup = step(&server.url, "seed", "/login");
    setup.extractions.push(json_extraction("token", "/token"));
    let mut clear = step(&server.url, "clear", "/empty");
    let mut optional = json_extraction("token", "/absent");
    optional.required = false;
    clear.extractions.push(optional);
    let dependent = step(&server.url, "dependent", "/forbidden/{{session:token}}");
    let mut input = config(&server.url, vec![clear, dependent]);
    input.scenarios.as_mut().unwrap().setup.push(setup);
    let snapshot = finish(input).await;
    let received = server.target.received.lock().await.clone();
    assert_eq!(
        received
            .iter()
            .map(|request| request.uri.as_str())
            .collect::<Vec<_>>(),
        ["/login", "/empty"]
    );
    let report = report(&snapshot);
    let clear = report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["step_id"] == "clear")
        .unwrap();
    let dependent = report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["step_id"] == "dependent")
        .unwrap();
    assert_eq!(clear["extraction_failures"], "0");
    assert_eq!(dependent["binding_failures"], "1");
    server.close().await;
}

#[tokio::test]
async fn extraction_overwrite_requires_explicit_permission() {
    let server = Server::start().await;
    let mut first = step(&server.url, "first", "/login");
    first.extractions.push(json_extraction("token", "/token"));
    let mut collision = step(&server.url, "collision", "/login");
    let mut extraction = json_extraction("token", "/token");
    extraction.overwrite = false;
    collision.extractions.push(extraction);
    let forbidden = step(&server.url, "skipped", "/forbidden");
    let snapshot = finish(config(&server.url, vec![first, collision, forbidden])).await;
    assert_eq!(snapshot.metrics.counters.responses, 2);
    let report = report(&snapshot);
    assert_eq!(report["steps"][1]["extraction_failures"], "1");
    assert_eq!(report["iterations"][0]["failed"], "1");
    assert_eq!(server.target.received.lock().await.len(), 2);
    server.close().await;
}

#[tokio::test]
async fn response_assertion_failure_uses_the_configured_step_failure_policy() {
    let server = Server::start().await;
    for policy in [StepFailure::Stop, StepFailure::Continue] {
        let mut checked = step(&server.url, "checked", "/empty");
        checked.on_failure = policy;
        checked.request.assertions.push(velo_engine::Assertion {
            id: "created".into(),
            rule: velo_engine::AssertionRule::Status { allowed: vec![201] },
        });
        let independent = step(&server.url, "independent", "/independent");
        let snapshot = finish(config(&server.url, vec![checked, independent])).await;
        assert_eq!(
            snapshot.metrics.counters.responses,
            if policy == StepFailure::Stop { 1 } else { 2 }
        );
        assert_eq!(report(&snapshot)["iterations"][0]["failed"], "1");
    }
    server.close().await;
}

#[tokio::test]
async fn stopping_during_setup_prevents_load_and_attempts_cleanup() {
    let server = Server::start().await;
    let mut setup = step(&server.url, "prepare", "/setup");
    setup.think_ms = 60_000;
    let mut input = config(&server.url, vec![step(&server.url, "load", "/forbidden")]);
    let plan = input.scenarios.as_mut().unwrap();
    plan.setup.push(setup);
    plan.cleanup.push(step(&server.url, "remove", "/cleanup"));
    let manager = RunManager::default();
    let started = manager.start(input, true).await.unwrap();
    timeout(Duration::from_secs(3), async {
        while server.target.received.lock().await.is_empty() {
            sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    manager.stop(&started.run_id).await.unwrap();
    let snapshot = terminal(&manager).await;
    assert_eq!(snapshot.state, RunState::Cancelled);
    assert_eq!(snapshot.metrics.counters.started, 0);
    assert_eq!(report(&snapshot)["setup"], "cancelled");
    assert_eq!(snapshot.arrival.offered, 0);
    assert_eq!(snapshot.arrival.missed_deadlines, 0);
    assert_eq!(snapshot.metrics.counters.completed_iterations, 0);
    assert_eq!(report(&snapshot)["iterations"][0]["started"], "0");
    assert_eq!(report(&snapshot)["cleanup"], "succeeded");
    let received = server.target.received.lock().await.clone();
    assert_eq!(
        received
            .iter()
            .map(|request| request.uri.as_str())
            .collect::<Vec<_>>(),
        ["/setup", "/cleanup"]
    );
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn expired_drain_cancels_owned_http_attempt_before_cleanup() {
    let server = Server::start().await;
    let mut input = config(&server.url, vec![step(&server.url, "slow", "/drain")]);
    input.workload = Workload::Constant {
        vus: 1,
        duration_ms: 60_000,
    };
    input.grace_ms = 0;
    input
        .scenarios
        .as_mut()
        .unwrap()
        .cleanup
        .push(step(&server.url, "remove", "/cleanup"));
    let manager = RunManager::default();
    let started = manager.start(input, true).await.unwrap();
    timeout(Duration::from_secs(3), async {
        while server.target.received.lock().await.is_empty() {
            sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    manager.stop(&started.run_id).await.unwrap();
    let snapshot = terminal(&manager).await;
    assert_eq!(snapshot.metrics.counters.cancelled, 1);
    assert_eq!(snapshot.metrics.counters.completed_iterations, 0);
    assert_eq!(report(&snapshot)["iterations"][0]["cancelled"], "1");
    assert_eq!(report(&snapshot)["cleanup"], "succeeded");
    manager.shutdown().await;
    server.close().await;
}

#[test]
fn scenario_preparation_rejects_ambiguous_identities_and_unbounded_shapes() {
    let base = "http://127.0.0.1:8080";
    let duplicate = step(base, "same", "/");
    assert!(RunManager::prepare(config(base, vec![duplicate.clone(), duplicate])).is_err());
    let mut input = config(base, vec![step(base, "valid", "/")]);
    let plan = input.scenarios.as_mut().unwrap();
    plan.scenarios.push(plan.scenarios[0].clone());
    assert!(RunManager::prepare(input).is_err());
    let mut input = config(base, vec![step(base, "valid", "/")]);
    input.scenarios.as_mut().unwrap().scenarios[0].weight = 0;
    assert!(RunManager::prepare(input).is_err());
    let mut input = config(base, vec![step(base, "valid", "/")]);
    input.scenarios.as_mut().unwrap().scenarios[0].steps.clear();
    assert!(RunManager::prepare(input).is_err());
    let mut input = config(base, vec![step(base, "valid", "/")]);
    input.scenarios.as_mut().unwrap().scenarios[0].steps[0].think_ms = 3_600_001;
    assert!(RunManager::prepare(input).is_err());
}

#[tokio::test]
async fn cleanup_delay_does_not_dilute_measured_load_duration_or_rates() {
    let server = Server::start().await;
    let mut input = config(&server.url, vec![step(&server.url, "load", "/load")]);
    input
        .scenarios
        .as_mut()
        .unwrap()
        .cleanup
        .push(step(&server.url, "remove", "/cleanup-slow"));
    let started = Instant::now();
    let snapshot = finish(input).await;
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(350));
    assert!(
        elapsed
            .as_micros()
            .saturating_sub(u128::from(snapshot.actual_us))
            >= 250_000
    );
    assert_eq!(snapshot.metrics.counters.responses, 1);
    assert_eq!(snapshot.metrics.counters.completed_iterations, 1);
    let expected_rate = 1_000_000.0 / snapshot.actual_us as f64;
    assert!((snapshot.metrics.request_rate - expected_rate).abs() < 0.001);
    assert!((snapshot.metrics.iteration_rate - expected_rate).abs() < 0.001);
    assert_eq!(report(&snapshot)["cleanup"], "succeeded");
    server.close().await;
}

#[tokio::test]
async fn cookies_respect_paths_domains_and_redirects() {
    let server = Server::start().await;
    let steps = vec![
        step(&server.url, "set", "/cookie-set"),
        step(&server.url, "inside", "/inside/echo"),
        step(&server.url, "outside", "/outside"),
        step(&server.url, "redirect", "/cookie-redirect"),
        step(&server.url, "after-redirect", "/inside/after"),
    ];
    let mut input = config(&server.url, steps);
    input.max_redirects = 1;
    let snapshot = finish(input).await;
    let received = server.target.received.lock().await.clone();
    assert_eq!(received.len(), 6);
    assert_eq!(received[1].cookie.as_deref(), Some("scoped=inside"));
    assert!(received[2].cookie.is_none());
    assert!(received[3].cookie.is_none());
    for request in &received[4..] {
        let cookie = request.cookie.as_deref().unwrap();
        assert!(cookie.contains("scoped=inside"));
        assert!(cookie.contains("redirect=retained"));
        assert!(!cookie.contains("secure="));
        assert!(!cookie.contains("foreign="));
    }
    assert_eq!(snapshot.metrics.counters.responses, 5);
    assert_eq!(snapshot.metrics.counters.completed_iterations, 1);
    server.close().await;
}

#[tokio::test]
async fn response_cookie_count_limit_fails_the_step_without_continuing() {
    let server = Server::start().await;
    let snapshot = finish(config(
        &server.url,
        vec![
            step(&server.url, "overflow", "/cookie-overflow"),
            step(&server.url, "forbidden", "/forbidden"),
        ],
    ))
    .await;
    assert_eq!(server.target.received.lock().await.len(), 1);
    let report = report(&snapshot);
    assert_eq!(report["steps"][0]["last_error"], "SESSION_BUDGET_EXCEEDED");
    assert_eq!(report["iterations"][0]["failed"], "1");
    server.close().await;
}

#[tokio::test]
async fn real_dataset_definitions_bind_one_selected_row_across_all_iteration_steps() {
    let server = Server::start().await;
    let mut login = step(&server.url, "login", "/login");
    login.request.method = RequestMethod::POST;
    login.request.body =
        RequestBody::Json(r#"{"id":"{{id}}","value":"{{value}}","rowToken":"{{token}}"}"#.into());
    login.extractions.push(json_extraction("token", "/token"));
    let mut dependent = step(&server.url, "record", "/dataset/{{id}}");
    dependent.request.method = RequestMethod::POST;
    dependent.request.auth = RequestAuth::Bearer {
        token: "{{session:token}}".into(),
    };
    dependent.request.body = RequestBody::Json(
        r#"{"id":"{{id}}","value":"{{value}}","rowToken":"{{token}}","sessionToken":"{{session:token}}"}"#.into(),
    );
    let mut input = config(&server.url, vec![login, dependent]);
    input.workload = Workload::Constant {
        vus: 1,
        duration_ms: 1000,
    };
    input.pacing_ms = 5;
    let definition = TestDefinition {
        config: input,
        dataset: Some(DataDefinition {
            lifetime: DataLifetime::Iteration,
            allocation: DataAllocation::Unique,
            exhaustion: DataExhaustion::Stop,
            seed: 42,
            source: DataSource::Json {
                mapping: BTreeMap::from([
                    ("id".into(), "id".into()),
                    ("value".into(), "value".into()),
                    ("token".into(), "token".into()),
                ]),
            },
        }),
        variables: BTreeMap::from([
            ("id".into(), "shared".into()),
            ("token".into(), "shared".into()),
        ]),
        environments: BTreeMap::from([(
            "staging".into(),
            BTreeMap::from([
                ("id".into(), "environment".into()),
                ("token".into(), "environment".into()),
            ]),
        )]),
        ..Default::default()
    };
    let rows: Vec<_> = (0..3)
        .map(|index| {
            BTreeMap::from([
                ("id".into(), format!("record-{index}")),
                ("value".into(), format!("PRIVATE_ROW_{index}_\"é")),
                ("token".into(), format!("dataset-{index}")),
            ])
        })
        .collect();
    let input = definition
        .resolve_data(
            Some("staging"),
            &rows,
            || false,
            |_| panic!("No secret lookups are expected for literal dataset values"),
        )
        .unwrap();
    let snapshot = finish(input).await;
    let received = server.target.received.lock().await.clone();
    assert_eq!(received.len(), 6);
    for (index, pair) in received.as_chunks::<2>().0.iter().enumerate() {
        assert_eq!(pair[0].uri, "/login");
        assert_eq!(pair[1].uri, format!("/dataset/record%2D{index}"));
        let login: serde_json::Value = serde_json::from_slice(&pair[0].body).unwrap();
        let record: serde_json::Value = serde_json::from_slice(&pair[1].body).unwrap();
        assert_eq!(login["id"], rows[index]["id"]);
        assert_eq!(login["value"], rows[index]["value"]);
        assert_eq!(login["rowToken"], rows[index]["token"]);
        assert_eq!(record["id"], login["id"]);
        assert_eq!(record["value"], login["value"]);
        assert_eq!(record["rowToken"], login["rowToken"]);
        let token = format!("user-{}", index + 1);
        assert_eq!(record["sessionToken"], token);
        assert_eq!(
            pair[1].authorization.as_deref(),
            Some(format!("Bearer {token}").as_str())
        );
        assert_eq!(
            pair[1].cookie.as_deref(),
            Some(format!("sid={token}").as_str())
        );
    }
    assert_eq!(snapshot.metrics.counters.responses, 6);
    assert_eq!(snapshot.metrics.counters.completed_iterations, 3);
    assert!(snapshot.dataset.as_ref().unwrap().exhausted);
    assert!(
        !serde_json::to_string(&snapshot)
            .unwrap()
            .contains("PRIVATE_ROW")
    );
    server.close().await;
}

#[tokio::test]
async fn multipart_steps_reuse_prepared_files_and_bind_session_fields() {
    use std::io::Write;
    let server = Server::start().await;
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"scenario-upload-content").unwrap();
    file.flush().unwrap();
    let mut login = step(&server.url, "login", "/login");
    login.extractions = vec![Extraction {
        name: "token".into(),
        source: ExtractionSource::Json {
            pointer: "/token".into(),
        },
        required: true,
        overwrite: false,
    }];
    let mut upload = step(&server.url, "upload", "/upload");
    upload.request.method = RequestMethod::POST;
    upload.request.body = RequestBody::Multipart(vec![
        velo_engine::MultipartPart::Text {
            name: "token".into(),
            value: "{{session:token}}".into(),
        },
        velo_engine::MultipartPart::File {
            name: "file".into(),
            path: file.path().to_str().unwrap().into(),
            filename: "{{session:token}}.txt".into(),
            content_type: "text/plain".into(),
        },
    ]);
    let snapshot = finish(config(&server.url, vec![login, upload])).await;
    assert_eq!(snapshot.state, RunState::Completed);
    let received = server.target.received.lock().await;
    assert_eq!(received.len(), 2);
    let body = String::from_utf8_lossy(&received[1].body);
    assert!(body.contains("scenario-upload-content"));
    assert!(body.contains("user-1.txt"));
    assert!(body.contains("user-1"));
    assert_eq!(snapshot.metrics.counters.responses, 2);
    drop(received);
    server.close().await;
}

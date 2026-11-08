use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tauri::{
    App, Listener, WebviewWindow,
    ipc::{CallbackFn, InvokeBody},
    test::{MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets},
    webview::InvokeRequest,
};
use velo_engine::{
    ApiCollection, CollectionEntry, Extraction, ExtractionSource, RunConfig, TestDefinition,
};

use super::{ApiState, DesktopRuns};

struct Target {
    origin: String,
    received: mpsc::Receiver<String>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Target {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let (send, received) = mpsc::channel();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            while !stopped.load(Ordering::Acquire) && Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut bytes = Vec::new();
                        let mut buffer = [0; 1024];
                        while bytes.len() < 32_768
                            && !bytes.windows(4).any(|window| window == b"\r\n\r\n")
                        {
                            match stream.read(&mut buffer) {
                                Ok(0) | Err(_) => break,
                                Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                            }
                        }
                        let request = String::from_utf8(bytes).unwrap();
                        let path = request.split_whitespace().nth(1).unwrap_or("/");
                        let status = if path == "/fail" {
                            "500 Internal Server Error"
                        } else if path == "/notfound" {
                            "404 Not Found"
                        } else {
                            "200 OK"
                        };
                        let cookie = if path == "/first" {
                            "Set-Cookie: session=private-cookie; Path=/\r\n"
                        } else {
                            ""
                        };
                        let body = r#"{"id":"42","ok":true}"#;
                        let response = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{cookie}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = send.send(request);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("test target accept failed: {error}"),
                }
            }
        });
        Self {
            origin,
            received,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

struct Workspace {
    window: WebviewWindow<MockRuntime>,
    runs: Arc<DesktopRuns>,
    _app: App<MockRuntime>,
    _directory: tempfile::TempDir,
}

impl Workspace {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let runs = Arc::new(DesktopRuns::default());
        let app = mock_builder()
            .manage(ApiState::default())
            .manage(runs.clone())
            .manage(crate::history::History::new(Some(
                directory.path().join("history.sqlite3"),
            )))
            .invoke_handler(tauri::generate_handler![
                super::run_api_collection,
                super::cancel_api_run,
                super::api_session,
                super::clear_api_session,
                crate::api_storage::api_history,
            ])
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        Self {
            window,
            runs,
            _app: app,
            _directory: directory,
        }
    }
}

fn invoke(window: &WebviewWindow<MockRuntime>, command: &str, body: Value) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|response| response.deserialize().unwrap())
}

fn request(id: &str, origin: &str, path: &str) -> CollectionEntry {
    CollectionEntry::Request {
        id: id.into(),
        name: id.into(),
        description: String::new(),
        notes: String::new(),
        definition: Box::new(TestDefinition {
            config: RunConfig {
                url: format!("{origin}{path}"),
                timeout_ms: 2_000,
                ..Default::default()
            },
            ..Default::default()
        }),
        inherit_auth: true,
        extractions: Vec::new(),
    }
}

fn arguments(collection: &ApiCollection) -> Value {
    json!({ "operationId": uuid::Uuid::new_v4().to_string(), "collection": collection,
        "selection": [], "environment": null, "iterations": 1, "delayMs": 0,
        "stopOnFailure": true, "authorized": true, "interactive": false, "rows": [] })
}

#[test]
fn collection_runner_keeps_displayed_order_and_reconciles_stop_and_continue_outcomes() {
    let workspace = Workspace::new();
    let target = Target::new();
    let collection = ApiCollection {
        entries: vec![
            request("first", &target.origin, "/first"),
            request("failure", &target.origin, "/fail"),
            request("last", &target.origin, "/last"),
        ],
        ..Default::default()
    };
    let stopped = invoke(
        &workspace.window,
        "run_api_collection",
        arguments(&collection),
    )
    .unwrap();
    assert_eq!(stopped["kind"], "functional");
    assert_eq!(stopped["total"], 3);
    assert_eq!(stopped["results"][0]["state"], "passed");
    assert_eq!(stopped["results"][1]["state"], "failed");
    assert_eq!(stopped["results"][2]["state"], "skipped");
    let first_sent = target
        .received
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(
        first_sent.starts_with("GET /first "),
        "unexpected request: {first_sent:?}"
    );
    assert!(
        target
            .received
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .starts_with("GET /fail ")
    );
    assert!(target.received.try_recv().is_err());
    let mut continued_args = arguments(&collection);
    continued_args["stopOnFailure"] = json!(false);
    let continued = invoke(&workspace.window, "run_api_collection", continued_args).unwrap();
    assert_eq!(continued["results"][2]["state"], "passed");
    for path in ["/first", "/fail", "/last"] {
        let request = target
            .received
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert!(request.starts_with(&format!("GET {path} ")));
    }
    let history = invoke(&workspace.window, "api_history", json!({})).unwrap();
    assert_eq!(history.as_array().unwrap().len(), 2);
    assert!(
        history[0]["results"][0]["response"]["body"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert!(!history.to_string().contains("private-cookie"));
}

#[test]
fn invalid_later_request_and_unauthorized_runs_send_no_traffic() {
    let workspace = Workspace::new();
    let target = Target::new();
    let mut collection = ApiCollection {
        entries: vec![
            request("first", &target.origin, "/first"),
            request("invalid", &target.origin, "/invalid"),
        ],
        ..Default::default()
    };
    let mut unauthorized = arguments(&collection);
    unauthorized["authorized"] = json!(false);
    assert_eq!(
        invoke(&workspace.window, "run_api_collection", unauthorized).unwrap_err()["code"],
        "AUTHORIZATION_REQUIRED"
    );
    if let CollectionEntry::Request { definition, .. } = &mut collection.entries[1] {
        definition.config.timeout_ms = 0;
    }
    let error = invoke(
        &workspace.window,
        "run_api_collection",
        arguments(&collection),
    )
    .unwrap_err();
    assert_eq!(error["code"], "INVALID_RUN_CONFIG");
    assert!(target.received.try_recv().is_err());
    let mut missing = collection;
    if let CollectionEntry::Request { definition, .. } = &mut missing.entries[1] {
        definition.config.timeout_ms = 2_000;
        definition.config.url = format!("{}/{{{{missing}}}}", target.origin);
    }
    assert_eq!(
        invoke(&workspace.window, "run_api_collection", arguments(&missing)).unwrap_err()["code"],
        "VARIABLE_UNRESOLVED"
    );
    assert!(target.received.try_recv().is_err());
}

#[test]
fn cancellation_during_delay_prevents_next_request_and_retains_partial_history() {
    let workspace = Workspace::new();
    let target = Target::new();
    let collection = ApiCollection {
        entries: vec![
            request("first", &target.origin, "/first"),
            request("last", &target.origin, "/last"),
        ],
        ..Default::default()
    };
    let mut args = arguments(&collection);
    args["delayMs"] = json!(60_000);
    let operation_id = args["operationId"].clone();
    let (send, receive) = mpsc::channel();
    workspace.window.listen("api-progress", move |event| {
        let _ = send.send(serde_json::from_str::<Value>(event.payload()).unwrap());
    });
    let window = workspace.window.clone();
    let running = thread::spawn(move || invoke(&window, "run_api_collection", args));
    let update = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(update["results"][0]["state"], "passed");
    invoke(
        &workspace.window,
        "cancel_api_run",
        json!({"operationId":operation_id}),
    )
    .unwrap();
    let report = running.join().unwrap().unwrap();
    assert_eq!(report["state"], "cancelled");
    assert_eq!(report["results"][1]["state"], "skipped");
    let first_sent = target
        .received
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(
        first_sent.starts_with("GET /first "),
        "unexpected request: {first_sent:?}"
    );
    assert!(target.received.try_recv().is_err());
    let history = invoke(&workspace.window, "api_history", json!({})).unwrap();
    assert_eq!(history[0]["state"], "cancelled");
    assert_eq!(history[0]["results"].as_array().unwrap().len(), 2);
}

#[test]
fn active_load_run_and_shared_admission_gate_reject_api_traffic() {
    let workspace = Workspace::new();
    let target = Target::new();
    let collection = ApiCollection {
        entries: vec![request("api", &target.origin, "/api")],
        ..Default::default()
    };
    let admission = tauri::async_runtime::block_on(workspace.runs.admission.lock());
    assert_eq!(
        invoke(
            &workspace.window,
            "run_api_collection",
            arguments(&collection)
        )
        .unwrap_err()["code"],
        "REQUEST_BUSY"
    );
    drop(admission);
    let load = RunConfig {
        url: format!("{}/load", target.origin),
        workload: velo_engine::Workload::Constant {
            vus: 1,
            duration_ms: 30_000,
        },
        grace_ms: 0,
        ..Default::default()
    };
    tauri::async_runtime::block_on(workspace.runs.manager.start(load, true)).unwrap();
    assert_eq!(
        invoke(
            &workspace.window,
            "run_api_collection",
            arguments(&collection)
        )
        .unwrap_err()["code"],
        "RUN_ALREADY_ACTIVE"
    );
    tauri::async_runtime::block_on(workspace.runs.manager.shutdown());
    for request in target.received.try_iter() {
        assert!(!request.starts_with("GET /api "));
    }
}

#[test]
fn interactive_sessions_are_isolated_by_collection_environment_and_independent_runs() {
    let workspace = Workspace::new();
    let target = Target::new();
    let mut first = request("first", &target.origin, "/first");
    if let CollectionEntry::Request { extractions, .. } = &mut first {
        extractions.push(Extraction {
            name: "id".into(),
            source: ExtractionSource::Json {
                pointer: "/id".into(),
            },
            required: true,
            overwrite: true,
        });
    }
    let mut collection = ApiCollection {
        entries: vec![
            first,
            request("next", &target.origin, "/items/{{session:id}}"),
        ],
        ..Default::default()
    };
    for name in ["staging", "production"] {
        collection
            .environments
            .insert(name.into(), Default::default());
    }
    let mut first_args = arguments(&collection);
    first_args["selection"] = json!(["first"]);
    first_args["environment"] = json!("staging");
    first_args["interactive"] = json!(true);
    let first_report = invoke(&workspace.window, "run_api_collection", first_args).unwrap();
    assert_eq!(first_report["results"][0]["state"], "passed");
    let first_sent = target
        .received
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(
        first_sent.starts_with("GET /first "),
        "unexpected request: {first_sent:?}"
    );
    let staged = invoke(
        &workspace.window,
        "api_session",
        json!({"collectionId":collection.id,"environment":"staging"}),
    )
    .unwrap();
    assert_eq!(staged["variables"], json!(["id"]));
    assert_eq!(staged["cookies"].as_array().unwrap().len(), 1);
    assert!(!staged.to_string().contains("private-cookie"));
    let mut next_args = arguments(&collection);
    next_args["selection"] = json!(["next"]);
    next_args["environment"] = json!("staging");
    next_args["interactive"] = json!(true);
    assert_eq!(
        invoke(&workspace.window, "run_api_collection", next_args.clone()).unwrap()["results"][0]["state"],
        "passed"
    );
    let sent = target
        .received
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(sent.starts_with("GET /items/42 "));
    assert!(
        sent.to_ascii_lowercase()
            .contains("cookie: session=private-cookie")
    );
    for (environment, interactive, other_collection) in [
        ("production", true, false),
        ("staging", false, false),
        ("staging", true, true),
    ] {
        let mut args = next_args.clone();
        args["environment"] = json!(environment);
        args["interactive"] = json!(interactive);
        if other_collection {
            args["collection"]["id"] = json!("other-collection");
        }
        let report = invoke(&workspace.window, "run_api_collection", args).unwrap();
        assert_eq!(report["results"][0]["state"], "failed");
        assert!(target.received.try_recv().is_err());
    }
    invoke(
        &workspace.window,
        "clear_api_session",
        json!({"collectionId":collection.id,"environment":"staging"}),
    )
    .unwrap();
    let cleared = invoke(
        &workspace.window,
        "api_session",
        json!({"collectionId":collection.id,"environment":"staging"}),
    )
    .unwrap();
    assert!(cleared["variables"].as_array().unwrap().is_empty());
    assert!(cleared["cookies"].as_array().unwrap().is_empty());
}

#[test]
fn expected_error_status_passes_shared_checks_while_unexpected_status_still_fails() {
    let workspace = Workspace::new();
    let target = Target::new();
    let mut expected = request("expected", &target.origin, "/notfound");
    if let CollectionEntry::Request { definition, .. } = &mut expected {
        definition.config.assertions.push(velo_engine::Assertion {
            id: "expected-error".into(),
            rule: velo_engine::AssertionRule::Status { allowed: vec![404] },
        });
    }
    let mut wrong_expectation = request("wrong-expectation", &target.origin, "/notfound");
    if let CollectionEntry::Request { definition, .. } = &mut wrong_expectation {
        definition.config.assertions.push(velo_engine::Assertion {
            id: "expected-success".into(),
            rule: velo_engine::AssertionRule::Status { allowed: vec![200] },
        });
    }
    let collection = ApiCollection {
        entries: vec![
            expected,
            request("unasserted", &target.origin, "/notfound"),
            wrong_expectation,
        ],
        ..Default::default()
    };
    let mut args = arguments(&collection);
    args["stopOnFailure"] = json!(false);
    let report = invoke(&workspace.window, "run_api_collection", args).unwrap();
    assert_eq!(report["results"][0]["state"], "passed");
    assert_eq!(report["results"][0]["response"]["status"], 404);
    assert_eq!(
        report["results"][0]["response"]["checks"],
        json!(["passed"])
    );
    assert_eq!(report["results"][1]["state"], "failed");
    assert_eq!(report["results"][2]["state"], "failed");
    assert_eq!(
        report["results"][2]["response"]["checks"],
        json!(["status_mismatch"])
    );
}

#[test]
fn dataset_stop_exhaustion_prevents_all_later_collection_steps() {
    let workspace = Workspace::new();
    let target = Target::new();
    let mut first = request("first", &target.origin, "/first");
    if let CollectionEntry::Request { definition, .. } = &mut first {
        definition.dataset = Some(velo_engine::DataDefinition {
            lifetime: velo_engine::DataLifetime::Iteration,
            allocation: velo_engine::DataAllocation::Sequential,
            exhaustion: velo_engine::DataExhaustion::Stop,
            seed: 0,
            source: velo_engine::DataSource::Json {
                mapping: [("name".into(), "/name".into())].into(),
            },
        });
    }
    let collection = ApiCollection {
        entries: vec![first, request("last", &target.origin, "/last")],
        ..Default::default()
    };
    let mut args = arguments(&collection);
    args["iterations"] = json!(2);
    args["rows"] = json!([{"name":"only-row"}]);
    let report = invoke(&workspace.window, "run_api_collection", args).unwrap();
    assert_eq!(report["results"][0]["state"], "passed");
    assert_eq!(report["results"][1]["state"], "passed");
    assert_eq!(report["results"][2]["state"], "skipped");
    assert_eq!(report["results"][2]["error"], "DATASET_EXHAUSTED");
    assert_eq!(report["results"][3]["state"], "skipped");
    for path in ["/first", "/last"] {
        let sent = target
            .received
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert!(sent.starts_with(&format!("GET {path} ")));
    }
    assert!(target.received.try_recv().is_err());
}

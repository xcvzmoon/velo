use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    response::Response,
    routing::any,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex, Semaphore},
    task::{JoinHandle, JoinSet},
    time::{Instant, sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use velo_engine::{
    NameValue, RampStage, RequestBody, RequestMethod, RunConfig, RunManager, RunSnapshot, RunState,
    Workload,
};

#[derive(Clone, Debug)]
struct Received {
    method: String,
    uri: String,
    header: Option<String>,
    content_type: Option<String>,
    authorization: Option<String>,
    version: String,
    body: Vec<u8>,
}

#[derive(Clone)]
struct Target {
    received: Arc<Mutex<Option<Received>>>,
    requests: Arc<AtomicUsize>,
    connections: Arc<Mutex<HashSet<SocketAddr>>>,
    gate: Arc<Semaphore>,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            received: Arc::default(),
            requests: Arc::default(),
            connections: Arc::default(),
            gate: Arc::new(Semaphore::new(0)),
        }
    }
}

struct Server {
    url: String,
    stop: CancellationToken,
    task: JoinHandle<()>,
}

impl Server {
    async fn start() -> (Self, Target) {
        let target = Target::default();
        let app = Router::new()
            .fallback(any(handle))
            .with_state(target.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stop = CancellationToken::new();
        let shutdown = stop.clone();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
        });
        (Self { url, stop, task }, target)
    }

    async fn raw(response: &'static [u8], stall: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stop = CancellationToken::new();
        let shutdown = stop.clone();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown.cancelled() => break,
                    result = connections.join_next(), if !connections.is_empty() => { result.unwrap().unwrap(); }
                    result = listener.accept() => {
                        let (mut stream, _) = result.unwrap();
                        connections.spawn(async move {
                            let mut buffer = [0; 4096];
                            let _ = stream.read(&mut buffer).await;
                            if !response.is_empty() { let _ = stream.write_all(response).await; }
                            if stall { std::future::pending::<()>().await; }
                        });
                    }
                }
            }
            connections.abort_all();
            while connections.join_next().await.is_some() {}
        });
        Self { url, stop, task }
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

async fn handle(
    State(target): State<Target>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    target.connections.lock().await.insert(remote);
    target.requests.fetch_add(1, Ordering::SeqCst);
    let path = request.uri().path().to_owned();
    let (parts, body) = request.into_parts();
    let received = Received {
        authorization: parts
            .headers
            .get("authorization")
            .map(|value| value.to_str().unwrap().into()),
        version: format!("{:?}", parts.version),
        method: parts.method.to_string(),
        uri: parts.uri.to_string(),
        header: parts
            .headers
            .get("x-test")
            .map(|value| value.to_str().unwrap().into()),
        content_type: parts
            .headers
            .get("content-type")
            .map(|value| value.to_str().unwrap().into()),
        body: to_bytes(body, 1024 * 1024).await.unwrap().to_vec(),
    };
    *target.received.lock().await = Some(received);
    if let Some(status) = path.strip_prefix("/redirect/") {
        let status = status.parse::<u16>().unwrap();
        return Response::builder()
            .status(status)
            .header("location", "/echo")
            .body(Body::from("hop"))
            .unwrap();
    }
    if path == "/loop" {
        return Response::builder()
            .status(302)
            .header("location", "/loop")
            .body(Body::from("hop"))
            .unwrap();
    }
    if path == "/cross-origin" {
        return Response::builder()
            .status(307)
            .header("location", "http://127.0.0.1:1/secret")
            .body(Body::from("hop"))
            .unwrap();
    }
    if path == "/delayed-redirect" {
        sleep(Duration::from_millis(80)).await;
        return Response::builder()
            .status(307)
            .header("location", "/slow")
            .body(Body::from("hop"))
            .unwrap();
    }
    if path == "/held" {
        target.gate.acquire().await.unwrap().forget();
    }
    if path == "/slow" {
        sleep(Duration::from_millis(80)).await;
    }
    if path == "/large" {
        return Response::new(Body::from(vec![b'a'; 1024 * 1024]));
    }
    let status = if path == "/failure" {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    };
    Response::builder()
        .status(status)
        .body(Body::from("payload"))
        .unwrap()
}

fn config(url: String) -> RunConfig {
    RunConfig {
        url,
        workload: Workload::Constant {
            vus: 1,
            duration_ms: 150,
        },
        grace_ms: 100,
        pacing_ms: 5,
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
                reconcile(&snapshot);
                return snapshot;
            }
            sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap()
}

fn reconcile(snapshot: &RunSnapshot) {
    let counters = &snapshot.metrics.counters;
    assert_eq!(
        counters.started,
        counters.responses
            + counters.transport_failures
            + counters.timeouts
            + counters.cancelled
            + counters.in_flight
    );
    assert_eq!(
        counters.responses,
        counters.successes + counters.status_failures
    );
    assert_eq!(
        counters.completed_iterations,
        counters.responses + counters.transport_failures + counters.timeouts
    );
    assert_eq!(counters.in_flight, 0);
    assert_eq!(snapshot.active_vus, 0);
    assert!(snapshot.metrics.max_in_flight <= snapshot.confirmation.vus.into());
    assert_eq!(snapshot.desired_vus, 0);
    assert_eq!(snapshot.retiring_vus, 0);
    assert_eq!(
        snapshot.actual_us,
        snapshot.scheduling_us + snapshot.drain_us
    );
}

#[tokio::test]
async fn get_post_query_headers_json_and_text_are_constructed() {
    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    let mut input = config(format!("{}/echo?existing=yes", server.url));
    input.query = vec![
        NameValue {
            name: "q".into(),
            value: "a & b".into(),
        },
        NameValue {
            name: "q".into(),
            value: "second".into(),
        },
    ];
    input.headers = vec![NameValue {
        name: "x-test".into(),
        value: "custom".into(),
    }];
    manager.start(input.clone(), true).await.unwrap();
    let summary = terminal(&manager).await;
    assert!(summary.metrics.counters.successes > 0);
    assert_eq!(
        summary.metrics.counters.response_body_bytes,
        summary.metrics.counters.responses * 7
    );
    let received = target.received.lock().await.clone().unwrap();
    assert_eq!(received.method, "GET");
    assert_eq!(received.uri, "/echo?existing=yes&q=a+%26+b&q=second");
    assert_eq!(received.header.as_deref(), Some("custom"));
    assert!(summary.metrics.counters.responses > 1);
    assert_eq!(target.connections.lock().await.len(), 1);
    assert!(received.body.is_empty());
    for body in [
        RequestBody::Json("{\"ok\":true}".into()),
        RequestBody::Text("hello".into()),
    ] {
        input.method = RequestMethod::POST;
        input.body = body.clone();
        manager.start(input.clone(), true).await.unwrap();
        terminal(&manager).await;
        let received = target.received.lock().await.clone().unwrap();
        assert_eq!(received.method, "POST");
        match body {
            RequestBody::Json(value) => {
                assert_eq!(received.body, value.as_bytes());
                assert_eq!(received.content_type.as_deref(), Some("application/json"));
            }
            RequestBody::Text(value) => {
                assert_eq!(received.body, value.as_bytes());
                assert_eq!(
                    received.content_type.as_deref(),
                    Some("text/plain; charset=utf-8")
                );
            }
            RequestBody::None | RequestBody::Form(_) | RequestBody::Multipart(_) => unreachable!(),
        }
    }
    input.headers.push(NameValue {
        name: "content-type".into(),
        value: "application/custom".into(),
    });
    manager.start(input, true).await.unwrap();
    terminal(&manager).await;
    assert_eq!(
        target
            .received
            .lock()
            .await
            .as_ref()
            .unwrap()
            .content_type
            .as_deref(),
        Some("application/custom")
    );
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn status_transport_timeout_partial_body_and_redirect_populations() {
    let (server, _) = Server::start().await;
    let manager = RunManager::default();
    manager
        .start(config(format!("{}/failure", server.url)), true)
        .await
        .unwrap();
    let summary = terminal(&manager).await;
    assert!(summary.metrics.counters.status_failures > 0);
    assert_eq!(
        summary.metrics.response_latency.samples,
        summary.metrics.counters.responses
    );
    assert_eq!(summary.metrics.http_failure_rate, Some(1.0));

    let mut input = config(format!("{}/slow", server.url));
    input.timeout_ms = 10;
    manager.start(input, true).await.unwrap();
    let summary = terminal(&manager).await;
    assert!(summary.metrics.counters.timeouts > 0);
    assert_eq!(
        summary.metrics.timeout_latency.samples,
        summary.metrics.counters.timeouts
    );
    assert_eq!(summary.metrics.response_latency.samples, 0);

    let broken = Server::raw(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nabc", false).await;
    manager
        .start(config(broken.url.clone()), true)
        .await
        .unwrap();
    let summary = terminal(&manager).await;
    assert!(summary.metrics.counters.transport_failures > 0);
    assert_eq!(summary.metrics.counters.responses, 0);
    assert!(summary.metrics.counters.response_body_bytes > 0);
    assert_eq!(
        summary.metrics.transport_latency.samples,
        summary.metrics.counters.transport_failures
    );
    broken.close().await;

    let redirect = Server::raw(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/never\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc", false).await;
    manager
        .start(config(redirect.url.clone()), true)
        .await
        .unwrap();
    let summary = terminal(&manager).await;
    assert!(summary.metrics.counters.successes > 0);
    assert_eq!(summary.metrics.counters.transport_failures, 0);
    assert_eq!(
        summary.metrics.counters.response_body_bytes,
        summary.metrics.counters.responses * 3
    );
    redirect.close().await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    manager.start(config(url), true).await.unwrap();
    let summary = terminal(&manager).await;
    assert!(summary.metrics.counters.transport_failures > 0);
    assert_eq!(summary.metrics.counters.responses, 0);
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn large_bodies_are_fully_consumed_without_retaining_responses() {
    let (server, _) = Server::start().await;
    let manager = RunManager::default();
    manager
        .start(config(format!("{}/large", server.url)), true)
        .await
        .unwrap();
    let summary = terminal(&manager).await;
    assert!(summary.metrics.counters.responses > 0);
    assert_eq!(
        summary.metrics.counters.response_body_bytes,
        summary.metrics.counters.responses * 1024 * 1024
    );
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn stalled_target_is_bounded_cancelled_and_joined_without_backlog() {
    let server = Server::raw(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nabc", true).await;
    let manager = RunManager::default();
    let mut input = config(server.url.clone());
    input.workload = Workload::Constant {
        vus: 16,
        duration_ms: 30_000,
    };
    input.grace_ms = 20;
    manager.start(input.clone(), true).await.unwrap();
    timeout(Duration::from_secs(2), async {
        loop {
            if manager.current().await.unwrap().metrics.counters.in_flight == 16 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    sleep(Duration::from_millis(30)).await;
    assert_eq!(
        manager.current().await.unwrap().metrics.counters.started,
        16
    );
    assert_eq!(
        manager.start(input.clone(), true).await.unwrap_err().code,
        "RUN_ALREADY_ACTIVE"
    );
    assert_eq!(
        manager.stop("stale").await.unwrap_err().code,
        "RUN_NOT_FOUND"
    );
    let id = manager.current().await.unwrap().run_id;
    let stopping = manager.stop(&id).await.unwrap();
    let admitted = stopping.metrics.counters.started;
    assert_eq!(stopping.state, RunState::Stopping);
    let start = Instant::now();
    manager.shutdown().await;
    assert!(start.elapsed() < Duration::from_secs(1));
    manager.shutdown().await;
    let summary = terminal(&manager).await;
    assert_eq!(summary.state, RunState::Cancelled);
    assert_eq!(summary.metrics.counters.started, admitted);
    assert_eq!(summary.metrics.counters.cancelled, 16);
    assert_eq!(summary.metrics.counters.responses, 0);
    assert!(summary.metrics.counters.response_body_bytes > 0);
    assert_eq!(summary.metrics.response_latency.samples, 0);
    assert_eq!(summary.metrics.http_failure_rate, None);
    let frozen = serde_json::to_value(&summary).unwrap();
    sleep(Duration::from_millis(10)).await;
    assert_eq!(
        serde_json::to_value(manager.current().await.unwrap()).unwrap(),
        frozen
    );
    assert_eq!(manager.stop(&id).await.unwrap().state, RunState::Cancelled);
    input.workload = Workload::Constant {
        vus: 16,
        duration_ms: 30,
    };
    manager.start(input, true).await.unwrap();
    let repeated = terminal(&manager).await;
    assert_eq!(repeated.state, RunState::Completed);
    assert_ne!(repeated.run_id, id);
    assert_eq!(repeated.metrics.counters.cancelled, 16);
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn admission_races_stop_before_spawn_and_repeated_shutdown_are_safe() {
    let (server, _) = Server::start().await;
    let manager = RunManager::default();
    for _ in 0..5 {
        let input = config(server.url.clone());
        let (first, second) = tokio::join!(
            manager.start(input.clone(), true),
            manager.start(input, true)
        );
        assert!(first.is_ok());
        assert_eq!(second.unwrap_err().code, "RUN_ALREADY_ACTIVE");
        manager.stop(&first.unwrap().run_id).await.unwrap();
        manager.shutdown().await;
        assert_eq!(terminal(&manager).await.state, RunState::Cancelled);
        manager.shutdown().await;
    }
    server.close().await;
}

#[tokio::test]
async fn duration_stops_admission_and_drain_keeps_completed_responses() {
    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    let mut input = config(format!("{}/slow", server.url));
    input.workload = Workload::Constant {
        vus: 1,
        duration_ms: 20,
    };
    input.grace_ms = 200;
    manager.start(input, true).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.state, RunState::Completed);
    assert_eq!(summary.metrics.counters.started, 1);
    assert_eq!(summary.metrics.counters.successes, 1);
    assert!(summary.drain_us >= 30_000);
    assert!(summary.metrics.response_latency.min_us.unwrap() >= 60_000);
    assert_eq!(target.requests.load(Ordering::SeqCst), 1);
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn validation_confirmation_and_authorization_never_send_traffic() {
    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    let base = config(format!("{}/private?token=secret", server.url));
    let confirmation = RunManager::prepare(base.clone()).unwrap();
    assert_eq!(confirmation.origin, server.url);
    assert!(confirmation.warnings.is_empty());
    assert_eq!(
        manager.start(base.clone(), false).await.unwrap_err().code,
        "AUTHORIZATION_REQUIRED"
    );
    let mut high = base.clone();
    high.workload = Workload::Constant {
        vus: 100,
        duration_ms: 150,
    };
    assert_eq!(
        RunManager::prepare(high).unwrap().warnings,
        ["HIGH_VU_COUNT"]
    );
    let mut invalid = Vec::new();
    for url in [
        "file:///tmp/data",
        "not a URL",
        "https://user:secret@example.com/",
        "https://example.com/#secret",
    ] {
        let mut input = base.clone();
        input.url = url.into();
        invalid.push((input, "INVALID_RUN_CONFIG"));
    }
    for vus in [0, 257] {
        let mut input = base.clone();
        input.workload = Workload::Constant {
            vus,
            duration_ms: 150,
        };
        invalid.push((input, "VU_BUDGET_EXCEEDED"));
    }
    for timeout_ms in [0, 3_600_001] {
        let mut input = base.clone();
        input.timeout_ms = timeout_ms;
        invalid.push((input, "INVALID_RUN_CONFIG"));
    }
    for (name, value) in [
        ("bad header", "secret"),
        ("x-test", "secret\r\ninjected: yes"),
        ("host", "secret"),
        ("content-length", "999"),
        ("transfer-encoding", "chunked"),
    ] {
        let mut input = base.clone();
        input.headers.push(NameValue {
            name: name.into(),
            value: value.into(),
        });
        invalid.push((input, "INVALID_RUN_CONFIG"));
    }
    let mut input = base.clone();
    input.workload = Workload::Constant {
        vus: 1,
        duration_ms: 0,
    };
    invalid.push((input, "INVALID_RUN_CONFIG"));
    let mut input = base.clone();
    input.budget.max_request_bytes = 1;
    invalid.push((input, "REQUEST_BUDGET_EXCEEDED"));
    let mut input = base.clone();
    input.body = RequestBody::Text("secret".into());
    invalid.push((input, "INVALID_RUN_CONFIG"));
    let mut input = base.clone();
    input.method = RequestMethod::POST;
    input.body = RequestBody::Json("{secret".into());
    invalid.push((input, "INVALID_RUN_CONFIG"));
    for (input, code) in invalid {
        let error = manager.start(input, true).await.unwrap_err();
        assert_eq!(error.code, code);
        assert!(!serde_json::to_string(&error).unwrap().contains("secret"));
    }
    assert!(manager.current().await.is_none());
    assert_eq!(target.requests.load(Ordering::SeqCst), 0);
    let mut oversized = base.clone();
    oversized.workload = Workload::Constant {
        vus: 10,
        duration_ms: 150,
    };
    oversized.budget.max_request_bytes = 1_000;
    oversized.method = RequestMethod::POST;
    oversized.body = RequestBody::Text("a".repeat(100));
    assert_eq!(
        manager.start(oversized, true).await.unwrap_err().code,
        "REQUEST_BUDGET_EXCEEDED"
    );
    let mut adjustable = base;
    adjustable.workload = Workload::Constant {
        vus: 257,
        duration_ms: 150,
    };
    adjustable.budget.max_vus = 257;
    assert_eq!(RunManager::prepare(adjustable).unwrap().vus, 257);
    server.close().await;
}

#[tokio::test(start_paused = true)]
async fn stop_before_supervisor_starts_sends_no_requests() {
    let manager = RunManager::default();
    let accepted = manager
        .start(config("http://127.0.0.1:1/".into()), true)
        .await
        .unwrap();
    manager.stop(&accepted.run_id).await.unwrap();
    manager.shutdown().await;
    let summary = manager.current().await.unwrap();
    assert_eq!(summary.state, RunState::Cancelled);
    assert_eq!(summary.metrics.counters.started, 0);
    reconcile(&summary);
}

#[tokio::test]
async fn body_timeout_includes_reading_and_preserves_encoded_partial_bytes() {
    let server = Server::raw(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nabc", true).await;
    let manager = RunManager::default();
    let mut input = config(server.url.clone());
    input.timeout_ms = 20;
    manager.start(input, true).await.unwrap();
    let summary = terminal(&manager).await;
    assert!(summary.metrics.counters.timeouts > 0);
    assert_eq!(summary.metrics.counters.responses, 0);
    assert_eq!(
        summary.metrics.counters.response_body_bytes,
        summary.metrics.counters.timeouts * 3
    );
    server.close().await;

    let encoded = Server::raw(b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 3\r\nConnection: close\r\n\r\nxyz", false).await;
    manager
        .start(config(encoded.url.clone()), true)
        .await
        .unwrap();
    let summary = terminal(&manager).await;
    assert!(summary.metrics.counters.successes > 0);
    assert_eq!(summary.metrics.counters.transport_failures, 0);
    assert_eq!(
        summary.metrics.counters.response_body_bytes,
        summary.metrics.counters.responses * 3
    );
    manager.shutdown().await;
    encoded.close().await;
}

#[tokio::test]
async fn pacing_does_not_queue_iterations_and_stop_can_drain_successfully() {
    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    let mut input = config(server.url.clone());
    input.pacing_ms = 1_000;
    manager.start(input, true).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.metrics.counters.started, 1);
    assert_eq!(target.requests.load(Ordering::SeqCst), 1);

    let mut input = config(format!("{}/slow", server.url));
    input.workload = Workload::Constant {
        vus: 1,
        duration_ms: 10_000,
    };
    input.grace_ms = 200;
    let accepted = manager.start(input, true).await.unwrap();
    timeout(Duration::from_secs(1), async {
        while target.requests.load(Ordering::SeqCst) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    manager.stop(&accepted.run_id).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.state, RunState::Cancelled);
    assert_eq!(summary.metrics.counters.started, 1);
    assert_eq!(summary.metrics.counters.successes, 1);
    assert_eq!(summary.metrics.counters.cancelled, 0);
    assert_eq!(target.requests.load(Ordering::SeqCst), 2);
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn aggregate_intervals_preserve_totals_and_reject_replaced_runs() {
    let (server, _) = Server::start().await;
    let manager = RunManager::default();
    let config = RunConfig {
        url: server.url.clone(),
        workload: Workload::Constant {
            vus: 2,
            duration_ms: 30_000,
        },
        pacing_ms: 1,
        grace_ms: 100,
        ..RunConfig::default()
    };
    let run = manager.start(config.clone(), true).await.unwrap();
    assert!(manager.sample("old").await.is_none());
    sleep(Duration::from_millis(60)).await;
    let first = manager.sample(&run.run_id).await.unwrap();
    assert_eq!(first.sequence, "1");
    assert!(first.interval_us > 0);
    assert!(first.interval_requests > 0);
    assert_eq!(first.interval_failures, 0);
    assert_eq!(first.response_latency.samples, first.interval_requests);
    manager.stop(&run.run_id).await.unwrap();
    manager.shutdown().await;
    let second = manager.sample(&run.run_id).await.unwrap();
    assert_eq!(second.sequence, "2");
    assert_eq!(second.snapshot.state, RunState::Cancelled);
    assert_eq!(second.snapshot.active_vus, 0);
    assert_eq!(second.snapshot.metrics.counters.in_flight, 0);
    assert_eq!(
        first.interval_requests + second.interval_requests,
        second.snapshot.metrics.counters.completed_iterations
    );
    assert_eq!(
        first.response_latency.samples + second.response_latency.samples,
        second.snapshot.metrics.response_latency.samples
    );
    assert_eq!(
        first.interval_us + second.interval_us,
        second.snapshot.actual_us
    );
    let empty = manager.sample(&run.run_id).await.unwrap();
    assert_eq!(empty.interval_requests, 0);
    assert_eq!(empty.response_latency.samples, 0);
    assert_eq!(
        empty.snapshot.metrics.counters.started,
        second.snapshot.metrics.counters.started
    );
    assert_eq!(empty.interval_us, 0);
    let next = manager.start(config, true).await.unwrap();
    assert!(manager.sample(&run.run_id).await.is_none());
    assert_eq!(manager.sample(&next.run_id).await.unwrap().sequence, "1");
    manager.shutdown().await;
    server.close().await;
}

async fn wait_for_run(
    manager: &RunManager,
    condition: impl Fn(&RunSnapshot) -> bool,
) -> RunSnapshot {
    timeout(Duration::from_secs(4), async {
        loop {
            let current = manager.current().await.unwrap();
            if condition(&current) {
                return current;
            }
            sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn ramp_down_finishes_held_responses_without_cancelling_or_readmitting_retirees() {
    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    let input = RunConfig {
        url: format!("{}/held", server.url),
        workload: Workload::Ramping {
            start_vus: 4,
            stages: vec![
                RampStage {
                    duration_ms: 200,
                    target_vus: 0,
                },
                RampStage {
                    duration_ms: 3_000,
                    target_vus: 0,
                },
            ],
        },
        grace_ms: 0,
        ..RunConfig::default()
    };
    let accepted = manager.start(input, true).await.unwrap();
    let retired = wait_for_run(&manager, |snapshot| {
        snapshot.desired_vus == 0 && snapshot.retiring_vus == 4
    })
    .await;
    assert_eq!(retired.active_vus, 4);
    assert_eq!(retired.metrics.counters.started, 4);
    assert_eq!(retired.metrics.counters.cancelled, 0);
    assert_eq!(retired.metrics.counters.in_flight, 4);
    target.gate.add_permits(4);
    let idle = wait_for_run(&manager, |snapshot| snapshot.active_vus == 0).await;
    assert_eq!(idle.state, RunState::Running);
    assert_eq!(idle.metrics.counters.responses, 4);
    assert_eq!(idle.metrics.counters.cancelled, 0);
    assert_eq!(idle.retiring_vus, 0);
    assert_eq!(target.requests.load(Ordering::SeqCst), 4);
    manager.stop(&accepted.run_id).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.state, RunState::Cancelled);
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn later_ramp_up_waits_for_retirees_and_stop_joins_all_workers() {
    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    let input = RunConfig {
        url: format!("{}/held", server.url),
        workload: Workload::Ramping {
            start_vus: 4,
            stages: vec![
                RampStage {
                    duration_ms: 200,
                    target_vus: 0,
                },
                RampStage {
                    duration_ms: 100,
                    target_vus: 4,
                },
                RampStage {
                    duration_ms: 3_000,
                    target_vus: 4,
                },
            ],
        },
        grace_ms: 0,
        ..RunConfig::default()
    };
    let accepted = manager.start(input, true).await.unwrap();
    let blocked = wait_for_run(&manager, |snapshot| {
        snapshot.desired_vus == 4 && snapshot.retiring_vus == 4
    })
    .await;
    assert_eq!(blocked.active_vus, 4);
    assert_eq!(blocked.metrics.counters.started, 4);
    assert_eq!(blocked.max_vus, 4);
    target.gate.add_permits(4);
    let replaced = wait_for_run(&manager, |snapshot| snapshot.metrics.counters.started == 8).await;
    assert_eq!(replaced.metrics.counters.responses, 4);
    assert_eq!(replaced.retiring_vus, 0);
    assert_eq!(replaced.active_vus, 4);
    assert_eq!(replaced.max_vus, 4);
    manager.stop(&accepted.run_id).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.state, RunState::Cancelled);
    assert_eq!(summary.metrics.counters.responses, 4);
    assert_eq!(summary.metrics.counters.cancelled, 4);
    assert_eq!(summary.metrics.counters.started, 8);
    manager.shutdown().await;
    target.gate.add_permits(8);
    server.close().await;
}

#[test]
fn ramp_validation_rejects_invalid_schedules_and_counts_peak_load() {
    for (start_vus, stages, code) in [
        (0, vec![], "INVALID_RUN_CONFIG"),
        (
            0,
            vec![RampStage {
                duration_ms: 0,
                target_vus: 1,
            }],
            "INVALID_RUN_CONFIG",
        ),
        (
            0,
            vec![
                RampStage {
                    duration_ms: u32::MAX,
                    target_vus: 0,
                },
                RampStage {
                    duration_ms: 1,
                    target_vus: 0,
                },
            ],
            "INVALID_RUN_CONFIG",
        ),
        (
            257,
            vec![RampStage {
                duration_ms: 1,
                target_vus: 0,
            }],
            "VU_BUDGET_EXCEEDED",
        ),
        (
            0,
            vec![RampStage {
                duration_ms: 1,
                target_vus: 257,
            }],
            "VU_BUDGET_EXCEEDED",
        ),
    ] {
        let input = RunConfig {
            url: "http://127.0.0.1:1/".into(),
            workload: Workload::Ramping { start_vus, stages },
            ..RunConfig::default()
        };
        assert_eq!(RunManager::prepare(input).unwrap_err().code, code);
    }
    let input = RunConfig {
        url: "http://127.0.0.1:1/".into(),
        workload: Workload::Ramping {
            start_vus: 0,
            stages: vec![
                RampStage {
                    duration_ms: 500,
                    target_vus: 100,
                },
                RampStage {
                    duration_ms: 500,
                    target_vus: 0,
                },
            ],
        },
        ..RunConfig::default()
    };
    let confirmation = RunManager::prepare(input.clone()).unwrap();
    assert_eq!(confirmation.vus, 100);
    assert_eq!(confirmation.duration_ms, 1_000);
    assert_eq!(confirmation.warnings, ["HIGH_VU_COUNT"]);
    let mut constrained = input;
    constrained.budget.max_request_bytes = 100;
    assert_eq!(
        RunManager::prepare(constrained).unwrap_err().code,
        "REQUEST_BUDGET_EXCEEDED"
    );
}

#[tokio::test]
async fn ramp_from_zero_completes_and_drains_slow_responses_then_runs_again() {
    let (server, _) = Server::start().await;
    let manager = RunManager::default();
    let input = RunConfig {
        url: format!("{}/slow", server.url),
        workload: Workload::Ramping {
            start_vus: 0,
            stages: vec![
                RampStage {
                    duration_ms: 200,
                    target_vus: 4,
                },
                RampStage {
                    duration_ms: 200,
                    target_vus: 0,
                },
            ],
        },
        grace_ms: 200,
        ..RunConfig::default()
    };
    let first = manager.start(input.clone(), true).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.state, RunState::Completed);
    assert!(summary.metrics.counters.responses > 0);
    assert_eq!(summary.metrics.counters.cancelled, 0);
    assert!(summary.max_vus <= 4);
    let second = manager.start(input, true).await.unwrap();
    assert_ne!(first.run_id, second.run_id);
    manager.stop(&second.run_id).await.unwrap();
    manager.shutdown().await;
    let summary = terminal(&manager).await;
    assert_eq!(summary.state, RunState::Cancelled);
    server.close().await;
}

#[tokio::test]
async fn retirement_interrupts_pacing_without_another_iteration() {
    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    let input = RunConfig {
        url: server.url.clone(),
        workload: Workload::Ramping {
            start_vus: 4,
            stages: vec![
                RampStage {
                    duration_ms: 200,
                    target_vus: 0,
                },
                RampStage {
                    duration_ms: 1_000,
                    target_vus: 0,
                },
            ],
        },
        pacing_ms: 30_000,
        ..RunConfig::default()
    };
    let accepted = manager.start(input, true).await.unwrap();
    let retired = wait_for_run(&manager, |snapshot| {
        snapshot.state == RunState::Running
            && snapshot.scheduling_us >= 200_000
            && snapshot.active_vus == 0
    })
    .await;
    assert_eq!(retired.desired_vus, 0);
    assert_eq!(retired.retiring_vus, 0);
    assert_eq!(retired.metrics.counters.started, 4);
    assert_eq!(retired.metrics.counters.responses, 4);
    assert_eq!(retired.metrics.counters.cancelled, 0);
    assert_eq!(target.requests.load(Ordering::SeqCst), 4);
    manager.stop(&accepted.run_id).await.unwrap();
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn arrival_overload_bounds_tasks_and_reconciles_drops_without_http_failures() {
    let server = Server::raw(b"", true).await;
    let manager = RunManager::default();
    let input = RunConfig {
        workload: Workload::Arrival {
            rate: 1_000,
            capacity: 3,
            duration_ms: 250,
        },
        pacing_ms: 0,
        grace_ms: 0,
        ..config(server.url.clone())
    };
    manager.start(input, true).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.state, RunState::Completed);
    assert_eq!(summary.arrival.offered, 250);
    assert_eq!(summary.metrics.counters.started, 3);
    assert_eq!(summary.metrics.counters.cancelled, 3);
    assert_eq!(summary.max_vus, 3);
    assert!(summary.arrival.capacity_drops > 0);
    assert_eq!(
        summary.arrival.offered,
        summary.metrics.counters.started
            + summary.arrival.capacity_drops
            + summary.arrival.missed_deadlines
    );
    assert_eq!(summary.metrics.http_failure_rate, None);
    let wire = serde_json::to_value(&summary).unwrap();
    assert_eq!(wire["arrival"]["offered"], "250");
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn arrival_fast_target_stop_and_repeated_runs_keep_accounting() {
    let (server, _) = Server::start().await;
    let manager = RunManager::default();
    let input = RunConfig {
        workload: Workload::Arrival {
            rate: 20,
            capacity: 4,
            duration_ms: 500,
        },
        pacing_ms: 0,
        ..config(server.url.clone())
    };
    manager.start(input.clone(), true).await.unwrap();
    let completed = terminal(&manager).await;
    assert_eq!(completed.arrival.offered, 10);
    assert!(completed.metrics.counters.successes >= 8);
    assert_eq!(completed.arrival.capacity_drops, 0);
    assert_eq!(
        completed.arrival.offered,
        completed.metrics.counters.started + completed.arrival.missed_deadlines
    );
    let started = manager.start(input, true).await.unwrap();
    sleep(Duration::from_millis(125)).await;
    manager.stop(&started.run_id).await.unwrap();
    manager.shutdown().await;
    let partial = manager.current().await.unwrap();
    reconcile(&partial);
    assert_eq!(partial.state, RunState::Cancelled);
    assert!(partial.arrival.offered < 10);
    assert_eq!(
        partial.arrival.offered,
        partial.metrics.counters.started
            + partial.arrival.capacity_drops
            + partial.arrival.missed_deadlines
    );
    server.close().await;
}

#[test]
fn arrival_validation_rejects_invalid_inputs_and_reports_rate_warning() {
    for (rate, capacity, duration_ms, pacing_ms) in [
        (0, 1, 100, 0),
        (1, 0, 100, 0),
        (1, 257, 100, 0),
        (1, 1, 0, 0),
        (1, 1, 100, 1),
    ] {
        let input = RunConfig {
            url: "http://127.0.0.1:1/".into(),
            workload: Workload::Arrival {
                rate,
                capacity,
                duration_ms,
            },
            pacing_ms,
            ..RunConfig::default()
        };
        assert!(RunManager::prepare(input).is_err());
    }
    let confirmation = RunManager::prepare(RunConfig {
        url: "http://127.0.0.1:1/".into(),
        workload: Workload::Arrival {
            rate: 1_000,
            capacity: 1,
            duration_ms: 1,
        },
        ..RunConfig::default()
    })
    .unwrap();
    assert_eq!(confirmation.warnings, vec!["HIGH_ARRIVAL_RATE"]);
}

#[tokio::test]
async fn final_thresholds_use_terminal_populations_without_changing_http_outcomes() {
    use velo_engine::{Threshold, ThresholdMetric, ThresholdOperator, ThresholdStatus};

    let (server, _) = Server::start().await;
    let manager = RunManager::default();
    let mut settings = config(format!("{}/failure", server.url));
    settings.thresholds = vec![
        Threshold {
            metric: ThresholdMetric::HttpFailurePercent,
            operator: ThresholdOperator::LessThan,
            limit: 100.0,
        },
        Threshold {
            metric: ThresholdMetric::RequestRate,
            operator: ThresholdOperator::GreaterThan,
            limit: 0.0,
        },
    ];
    let initial = manager.start(settings, true).await.unwrap();
    assert!(initial.thresholds.is_empty());
    let summary = terminal(&manager).await;
    assert_eq!(summary.state, RunState::Completed);
    assert_eq!(summary.thresholds[0].observed, Some(100.0));
    assert_eq!(summary.thresholds[0].status, ThresholdStatus::Failed);
    assert_eq!(summary.thresholds[1].status, ThresholdStatus::Passed);
    assert_eq!(
        summary.metrics.counters.responses,
        summary.metrics.counters.status_failures
    );

    let mut settings = config(server.url.clone());
    settings.thresholds = vec![Threshold {
        metric: ThresholdMetric::HttpFailurePercent,
        operator: ThresholdOperator::LessThan,
        limit: 1.0,
    }];
    let initial = manager.start(settings, true).await.unwrap();
    manager.stop(&initial.run_id).await.unwrap();
    let partial = terminal(&manager).await;
    assert_eq!(partial.state, RunState::Cancelled);
    assert_eq!(
        partial.thresholds[0].status,
        ThresholdStatus::InsufficientData
    );
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn remaining_methods_authentication_forms_and_forced_protocols_reach_the_target() {
    use velo_engine::{HttpProtocol, RequestAuth};

    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    for method in [
        RequestMethod::PUT,
        RequestMethod::PATCH,
        RequestMethod::DELETE,
        RequestMethod::HEAD,
        RequestMethod::OPTIONS,
    ] {
        let mut input = config(format!("{}/echo", server.url));
        input.method = method;
        input.body = if matches!(method, RequestMethod::HEAD) {
            RequestBody::None
        } else {
            RequestBody::Text("payload".into())
        };
        manager.start(input, true).await.unwrap();
        let summary = terminal(&manager).await;
        assert!(summary.metrics.counters.successes > 0);
        let received = target.received.lock().await.clone().unwrap();
        assert_eq!(received.method, format!("{method:?}"));
        if matches!(method, RequestMethod::HEAD) {
            assert!(received.body.is_empty());
            assert_eq!(summary.metrics.counters.response_body_bytes, 0);
        } else {
            assert_eq!(received.body, b"payload");
        }
    }
    for (auth, expected) in [
        (
            RequestAuth::Bearer {
                token: "private-token".into(),
            },
            "Bearer private-token",
        ),
        (
            RequestAuth::Basic {
                username: "user".into(),
                password: "pass".into(),
            },
            "Basic dXNlcjpwYXNz",
        ),
    ] {
        let mut input = config(format!("{}/echo", server.url));
        input.auth = auth;
        input.method = RequestMethod::POST;
        input.body = RequestBody::Form(vec![
            NameValue {
                name: "tag".into(),
                value: "a & b".into(),
            },
            NameValue {
                name: "tag".into(),
                value: "日本".into(),
            },
        ]);
        manager.start(input, true).await.unwrap();
        let summary = terminal(&manager).await;
        let received = target.received.lock().await.clone().unwrap();
        assert_eq!(received.authorization.as_deref(), Some(expected));
        assert_eq!(
            received.content_type.as_deref(),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(received.body, b"tag=a+%26+b&tag=%E6%97%A5%E6%9C%AC");
        assert!(
            !serde_json::to_string(&summary)
                .unwrap()
                .contains("private-token")
        );
        assert!(
            !serde_json::to_string(&summary)
                .unwrap()
                .contains("dXNlcjpwYXNz")
        );
    }
    for (protocol, version) in [
        (HttpProtocol::Auto, "HTTP/1.1"),
        (HttpProtocol::Http1, "HTTP/1.1"),
        (HttpProtocol::Http2, "HTTP/2.0"),
    ] {
        let mut input = config(server.url.clone());
        input.protocol = protocol;
        manager.start(input, true).await.unwrap();
        let summary = terminal(&manager).await;
        assert!(summary.metrics.counters.successes > 0);
        assert_eq!(
            target.received.lock().await.as_ref().unwrap().version,
            version
        );
    }
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn redirects_count_payloads_preserve_or_rewrite_methods_and_enforce_limits_and_deadlines() {
    use velo_engine::RequestAuth;

    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    for status in [301, 302, 303, 307, 308] {
        let mut input = config(format!("{}/redirect/{status}", server.url));
        input.method = RequestMethod::POST;
        input.body = RequestBody::Text("private-body".into());
        input.auth = RequestAuth::Bearer {
            token: "private-token".into(),
        };
        input.max_redirects = 1;
        manager.start(input, true).await.unwrap();
        let summary = terminal(&manager).await;
        assert_eq!(
            summary.metrics.counters.response_body_bytes,
            summary.metrics.counters.responses * 10
        );
        let received = target.received.lock().await.clone().unwrap();
        assert_eq!(received.uri, "/echo");
        assert_eq!(
            received.authorization.as_deref(),
            Some("Bearer private-token")
        );
        if status < 307 {
            assert_eq!(received.method, "GET");
            assert!(received.body.is_empty());
            assert!(received.content_type.is_none());
        } else {
            assert_eq!(received.method, "POST");
            assert_eq!(received.body, b"private-body");
        }
    }
    for path in ["loop", "cross-origin"] {
        let mut input = config(format!("{}/{path}", server.url));
        input.max_redirects = 2;
        manager.start(input, true).await.unwrap();
        let summary = terminal(&manager).await;
        assert_eq!(summary.metrics.counters.responses, 0);
        assert!(summary.metrics.counters.transport_failures > 0);
        assert_eq!(
            summary.metrics.counters.response_body_bytes,
            summary.metrics.counters.transport_failures * if path == "loop" { 9 } else { 3 }
        );
    }
    let mut input = config(format!("{}/delayed-redirect", server.url));
    input.max_redirects = 1;
    input.timeout_ms = 120;
    input.workload = Workload::Constant {
        vus: 1,
        duration_ms: 30,
    };
    input.grace_ms = 300;
    manager.start(input, true).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.metrics.counters.timeouts, 1);
    assert_eq!(summary.metrics.counters.response_body_bytes, 3);
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test]
async fn multipart_streams_files_replays_redirects_and_closes_files_after_cancellation() {
    use std::io::Write;
    use velo_engine::MultipartPart;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(&vec![b'x'; 64 * 1024]).unwrap();
    file.flush().unwrap();
    let path = file.path().to_str().unwrap().to_owned();
    let parts = vec![
        MultipartPart::Text {
            name: "tag".into(),
            value: "one".into(),
        },
        MultipartPart::Text {
            name: "tag".into(),
            value: "two".into(),
        },
        MultipartPart::File {
            name: "upload".into(),
            path: path.clone(),
            filename: "public.txt".into(),
            content_type: "text/plain".into(),
        },
    ];
    let (server, target) = Server::start().await;
    let manager = RunManager::default();
    for endpoint in ["echo", "redirect/307", "redirect/308"] {
        let mut input = config(format!("{}/{endpoint}", server.url));
        input.method = RequestMethod::POST;
        input.body = RequestBody::Multipart(parts.clone());
        input.max_redirects = 1;
        manager.start(input, true).await.unwrap();
        let summary = terminal(&manager).await;
        assert!(summary.metrics.counters.successes > 0);
        let received = target.received.lock().await.clone().unwrap();
        assert!(
            received
                .content_type
                .unwrap()
                .starts_with("multipart/form-data; boundary=")
        );
        let body = String::from_utf8(received.body).unwrap();
        assert!(body.contains("filename=\"public.txt\""));
        assert!(!body.contains(&path));
        assert!(body.contains(&"x".repeat(64 * 1024)));
        assert!(body.find("one").unwrap() < body.find("two").unwrap());
        assert!(!serde_json::to_string(&summary).unwrap().contains(&path));
    }
    server.close().await;
    file.as_file().set_len(128 * 1024 * 1024).unwrap();
    let stalled = Server::raw(b"", true).await;
    let mut input = config(stalled.url.clone());
    input.method = RequestMethod::POST;
    input.body = RequestBody::Multipart(parts);
    input.workload = Workload::Constant {
        vus: 1,
        duration_ms: 30_000,
    };
    input.grace_ms = 0;
    let accepted = manager.start(input, true).await.unwrap();
    wait_for_run(&manager, |snapshot| {
        snapshot.metrics.counters.in_flight == 1
    })
    .await;
    sleep(Duration::from_millis(50)).await;
    manager.stop(&accepted.run_id).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.metrics.counters.cancelled, 1);
    manager.shutdown().await;
    stalled.close().await;
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/usr/sbin/lsof")
            .args(["-a", "-p", &std::process::id().to_string(), "-Fn", "--"])
            .arg(std::fs::canonicalize(&path).unwrap())
            .output()
            .unwrap();
        assert!(output.status.success());
        let output = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            output.lines().filter(|line| line.starts_with('n')).count(),
            1,
            "only the test-owned temporary file handle remains"
        );
    }
    #[cfg(target_os = "linux")]
    {
        let canonical = std::fs::canonicalize(&path).unwrap();
        let mut open = 0;
        for entry in std::fs::read_dir("/proc/self/fd").unwrap() {
            if std::fs::read_link(entry.unwrap().path()).ok().as_ref() == Some(&canonical) {
                open += 1;
            }
        }
        assert_eq!(open, 1, "only the test-owned temporary file handle remains");
    }
}

#[tokio::test]
async fn forced_http2_rejects_http1_without_falling_back() {
    let server = Server::raw(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n", false).await;
    let manager = RunManager::default();
    let mut input = config(server.url.clone());
    input.protocol = velo_engine::HttpProtocol::Http2;
    input.timeout_ms = 100;
    manager.start(input, true).await.unwrap();
    let summary = terminal(&manager).await;
    assert_eq!(summary.metrics.counters.responses, 0);
    assert!(summary.metrics.counters.transport_failures > 0);
    manager.shutdown().await;
    server.close().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "opt-in release benchmark; runs for 30 seconds per repetition"]
async fn release_benchmark() {
    let (server, _) = Server::start().await;
    let manager = RunManager::default();

    for repetition in 0..4 {
        let input = RunConfig {
            url: server.url.clone(),
            workload: Workload::Constant {
                vus: 16,
                duration_ms: if repetition == 0 { 5_000 } else { 30_000 },
            },
            ..RunConfig::default()
        };
        manager.start(input, true).await.unwrap();
        let snapshot = timeout(Duration::from_secs(45), async {
            loop {
                let snapshot = manager.current().await.unwrap();
                if matches!(
                    snapshot.state,
                    RunState::Completed | RunState::Cancelled | RunState::Failed
                ) {
                    break snapshot;
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap();

        reconcile(&snapshot);
        assert_eq!(snapshot.state, RunState::Completed);
        assert!(snapshot.metrics.counters.successes > 0);
        println!(
            "VELO_BENCHMARK repetition={repetition} warmup={} snapshot={}",
            repetition == 0,
            serde_json::to_string(&snapshot).unwrap()
        );
    }

    manager.shutdown().await;
    server.close().await;
}

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::Response,
    routing::any,
};
use tokio::net::TcpListener;
use velo_engine::{
    ApiCancellation, ApiSession, Assertion, AssertionRule, BodyFormat, CheckOutcome, Extraction,
    ExtractionSource, RunConfig, send_api_request,
};

struct Server {
    url: String,
    calls: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn server() -> Server {
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .fallback(any(handle))
        .with_state(calls.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Server { url, calls, task }
}

async fn handle(State(calls): State<Arc<AtomicUsize>>, request: Request) -> Response {
    calls.fetch_add(1, Ordering::SeqCst);
    let path = request.uri().path();
    let mut response = Response::builder().status(StatusCode::OK);
    if path == "/slow" {
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    if path == "/redirect" {
        return response
            .status(302)
            .header("location", "/echo")
            .header("set-cookie", "sid=private-cookie; Path=/")
            .body(Body::empty())
            .unwrap();
    }
    let body = match path {
        "/seed" => {
            response = response.header("set-cookie", "sid=private-cookie; Path=/");
            r#"{"id":"private-value","token":"sensitive-response","ok":true}"#.to_owned()
        }
        "/echo" => format!(
            "{} {}",
            request.uri(),
            request
                .headers()
                .get("cookie")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("no-cookie")
        ),
        "/large" => "x".repeat(70_000),
        "/binary" => {
            response = response.header("content-type", "application/octet-stream");
            "binary".into()
        }
        "/encoded" => {
            response = response.header("content-encoding", "gzip");
            "encoded".into()
        }
        _ => "ok".into(),
    };
    response.body(Body::from(body)).unwrap()
}

fn config(server: &Server, path: &str) -> RunConfig {
    RunConfig {
        url: format!("{}{path}", server.url),
        ..Default::default()
    }
}
fn extraction() -> Extraction {
    Extraction {
        name: "id".into(),
        source: ExtractionSource::Json {
            pointer: "/id".into(),
        },
        required: true,
        overwrite: false,
    }
}

#[tokio::test]
async fn shares_extraction_cookie_and_assertion_rules_without_exposing_values() {
    let server = server().await;
    let mut session = ApiSession::default();
    let cancellation = ApiCancellation::default();
    let mut request = config(&server, "/seed");
    request.assertions = vec![Assertion {
        id: "ok".into(),
        rule: AssertionRule::JsonEquals {
            pointer: "/ok".into(),
            value: "true".into(),
        },
    }];
    let (response, error) =
        send_api_request(request, &[extraction()], &[], &mut session, &cancellation)
            .await
            .unwrap();
    assert!(error.is_none());
    assert_eq!(response.checks, vec![CheckOutcome::Passed]);
    assert!(!response.body.contains("private-value"));
    assert!(!response.body.contains("sensitive-response"));
    assert_eq!(session.view().variables, vec!["id"]);
    assert_eq!(session.view().cookies.len(), 1);
    assert!(
        !serde_json::to_string(&session.view())
            .unwrap()
            .contains("private-cookie")
    );
    let (response, _) = send_api_request(
        config(&server, "/echo?item={{session:id}}"),
        &[],
        &[],
        &mut session,
        &cancellation,
    )
    .await
    .unwrap();
    assert!(response.body.contains("[redacted]"));
    assert!(!response.body.contains("private-cookie"));
    assert!(!response.body.contains("private-value"));
    let (response, _) = send_api_request(
        config(&server, "/echo"),
        &[],
        &[],
        &mut ApiSession::default(),
        &cancellation,
    )
    .await
    .unwrap();
    assert!(response.body.contains("no-cookie"));
    assert_eq!(server.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn missing_bindings_and_invalid_uploads_do_not_send_and_cancellation_does_not_retry() {
    let server = server().await;
    let mut session = ApiSession::default();
    let cancel = ApiCancellation::default();
    let error = send_api_request(
        config(&server, "/{{session:missing}}"),
        &[],
        &[],
        &mut session,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "VARIABLE_UNRESOLVED");
    assert_eq!(server.calls.load(Ordering::SeqCst), 0);
    let cancellation = cancel.clone();
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancellation.cancel();
    });
    let error = send_api_request(config(&server, "/slow"), &[], &[], &mut session, &cancel)
        .await
        .unwrap_err();
    task.await.unwrap();
    assert_eq!(error.code, "API_CANCELLED");
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    assert!(
        send_api_request(config(&server, "/seed"), &[], &[], &mut session, &cancel)
            .await
            .is_err()
    );
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn inspection_withholds_binary_and_encoded_content_and_reports_truncation() {
    let server = server().await;
    let cancel = ApiCancellation::default();
    let mut session = ApiSession::default();
    let (response, _) =
        send_api_request(config(&server, "/binary"), &[], &[], &mut session, &cancel)
            .await
            .unwrap();
    assert!(matches!(response.body_format, BodyFormat::Binary));
    assert!(response.body.is_empty());
    let (response, _) =
        send_api_request(config(&server, "/encoded"), &[], &[], &mut session, &cancel)
            .await
            .unwrap();
    assert!(matches!(
        response.body_format,
        BodyFormat::UnsupportedEncoding
    ));
    let mut request = config(&server, "/large");
    request.assertions = vec![Assertion {
        id: "complete".into(),
        rule: AssertionRule::TextContains { value: "x".into() },
    }];
    let (response, error) = send_api_request(request, &[extraction()], &[], &mut session, &cancel)
        .await
        .unwrap();
    assert_eq!(response.body_limit, 65_536);
    assert!(response.truncated);
    assert!(response.received_bytes >= 65_536);
    assert_eq!(response.checks, vec![CheckOutcome::Truncated]);
    assert_eq!(error.as_deref(), Some("EXTRACTION_BODY_TOO_LARGE"));
    assert!(session.view().variables.is_empty());
}

#[tokio::test]
async fn redirects_reuse_session_and_follow_only_the_configured_policy() {
    let server = server().await;
    let mut request = config(&server, "/redirect");
    request.max_redirects = 1;
    let (response, _) = send_api_request(
        request,
        &[],
        &[],
        &mut ApiSession::default(),
        &ApiCancellation::default(),
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("[redacted]"));
    assert_eq!(server.calls.load(Ordering::SeqCst), 2);
    let (response, _) = send_api_request(
        config(&server, "/redirect"),
        &[],
        &[],
        &mut ApiSession::default(),
        &ApiCancellation::default(),
    )
    .await
    .unwrap();
    assert_eq!(response.status, 302);
    assert_eq!(server.calls.load(Ordering::SeqCst), 3);
}

#[test]
fn configured_sensitive_fields_require_complete_json() {
    let mut response = velo_engine::PreviewResponse {
        body: r#"{"user":{"ssn":"private"},"ok":true}"#.into(),
        body_limit: 65_536,
        ..Default::default()
    };
    velo_engine::redact_response_fields(&mut response, &["/user/ssn".into()]);
    assert!(!response.body.contains("private"));
    assert!(response.body.contains("[redacted]"));
    response.truncated = true;
    response.body = "partial-private".into();
    velo_engine::redact_response_fields(&mut response, &["/user/ssn".into()]);
    assert!(!response.body.contains("private"));
}

#[test]
fn dataset_lifetimes_and_exhaustion_reuse_shared_allocation() {
    use velo_engine::{
        DataAllocation, DataDefinition, DataExhaustion, DataLifetime, DataSource, api_row_indexes,
    };
    let mut definition = DataDefinition {
        lifetime: DataLifetime::Iteration,
        allocation: DataAllocation::Sequential,
        exhaustion: DataExhaustion::Stop,
        seed: 0,
        source: DataSource::Json {
            mapping: [("name".into(), "/name".into())].into(),
        },
    };
    assert_eq!(api_row_indexes(&definition, 2, 3).unwrap(), vec![0, 1]);
    definition.exhaustion = DataExhaustion::Repeat;
    assert_eq!(api_row_indexes(&definition, 2, 3).unwrap(), vec![0, 1, 0]);
    definition.exhaustion = DataExhaustion::Fail;
    assert_eq!(
        api_row_indexes(&definition, 2, 3).unwrap_err().code,
        "DATASET_EXHAUSTED"
    );
    definition.lifetime = DataLifetime::Run;
    assert_eq!(api_row_indexes(&definition, 2, 3).unwrap(), vec![0, 0, 0]);
    definition.lifetime = DataLifetime::User;
    assert_eq!(api_row_indexes(&definition, 2, 3).unwrap(), vec![0, 0, 0]);
}

#[tokio::test]
async fn binding_failure_removes_stale_extraction_targets_without_traffic() {
    let server = server().await;
    let mut session = ApiSession::default();
    let cancel = ApiCancellation::default();
    send_api_request(
        config(&server, "/seed"),
        &[extraction()],
        &[],
        &mut session,
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(session.view().variables, vec!["id"]);
    let error = send_api_request(
        config(&server, "/{{session:missing}}"),
        &[extraction()],
        &[],
        &mut session,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "VARIABLE_UNRESOLVED");
    assert!(session.view().variables.is_empty());
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn connection_failure_preserves_safe_measured_diagnostic_category() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/private-request-value",
        listener.local_addr().unwrap()
    );
    drop(listener);
    let error = send_api_request(
        RunConfig {
            url,
            ..Default::default()
        },
        &[],
        &[],
        &mut ApiSession::default(),
        &ApiCancellation::default(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "API_TRANSPORT_CONNECTION_REFUSED");
    assert!(!error.to_string().contains("private-request-value"));
}

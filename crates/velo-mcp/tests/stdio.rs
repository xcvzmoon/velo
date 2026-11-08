//! End-to-end MCP wire tests against the standalone executable and real pipe transport.

use std::{path::Path, process::Stdio, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
};
use velo_contracts::{
    HistoryEntry,
    reports::{ComparisonReport, RegressionVerdict, RunMetadata},
};
use velo_engine::{RunConfig, TestDefinition};

struct Session {
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
}

impl Session {
    async fn start(arguments: &[&Path]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_velo-mcp"))
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut session = Self {
            child,
            input,
            output,
        };
        let initialized = session
            .request(
                1,
                "initialize",
                json!({
                    "protocolVersion":"2025-11-25", "capabilities":{},
                    "clientInfo":{"name":"velo-integration-test","version":"1"},
                }),
            )
            .await;
        assert_eq!(initialized["result"]["serverInfo"]["name"], "velo-mcp");
        assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
        session
            .send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        session
    }

    async fn send(&mut self, value: Value) {
        self.input
            .write_all(format!("{value}\n").as_bytes())
            .await
            .unwrap();
        self.input.flush().await.unwrap();
    }

    async fn response(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(5), self.output.next_line())
            .await
            .unwrap()
            .unwrap()
            .expect("server must return a JSON-RPC response");
        serde_json::from_str(&line).expect("stdout must contain JSON-RPC only")
    }

    async fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await;
        let response = self.response().await;
        assert_eq!(response["id"], id);
        response
    }

    async fn tool(&mut self, id: u64, name: &str, arguments: Value) -> Value {
        self.request(id, "tools/call", json!({"name":name,"arguments":arguments}))
            .await
    }

    async fn close(self) {
        drop(self.input);
        let output = tokio::time::timeout(Duration::from_secs(5), self.child.wait_with_output())
            .await
            .unwrap()
            .unwrap();
        assert!(output.status.success(), "session should close successfully");
        assert!(
            output.stderr.is_empty(),
            "successful sessions must not log request data"
        );
    }
}

fn definition() -> TestDefinition {
    TestDefinition {
        config: RunConfig {
            url: "http://127.0.0.1:9/never-send".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[tokio::test]
async fn protocol_discovery_offline_preparation_and_resource_authority() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut session = Session::start(&[]).await;
    let tools = session.request(2, "tools/list", json!({})).await;
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 5);
    assert!(
        tools
            .iter()
            .all(|tool| tool["annotations"]["readOnlyHint"] == true)
    );
    assert!(
        tools
            .iter()
            .all(|tool| tool["annotations"]["openWorldHint"] == false)
    );
    assert!(
        !tools
            .iter()
            .any(|tool| tool["name"] == "propose_definition")
    );
    let template = session.tool(3, "new_draft", json!({})).await;
    assert_eq!(template["result"]["structuredContent"]["version"], 1);
    let mut draft = definition();
    draft.config.url = format!("http://{}/health", listener.local_addr().unwrap());
    let checked = session
        .tool(
            4,
            "validate_definition",
            json!({"source":serde_json::to_string(&draft).unwrap()}),
        )
        .await;
    assert_eq!(
        checked["result"]["structuredContent"]["preparation"],
        "ready"
    );
    assert_eq!(
        checked["result"]["structuredContent"]["execution_authorized"],
        false
    );
    assert!(listener.accept().is_err());
    draft.config.workload = velo_engine::Workload::Constant {
        vus: 0,
        duration_ms: 1000,
    };
    let checked = session
        .tool(
            5,
            "validate_definition",
            json!({"source":serde_json::to_string(&draft).unwrap()}),
        )
        .await;
    assert_eq!(
        checked["result"]["structuredContent"]["preparation"],
        "invalid"
    );
    let resources = session.request(6, "resources/list", json!({})).await;
    assert_eq!(
        resources["result"]["resources"].as_array().unwrap().len(),
        3
    );
    let schema = session
        .request(
            7,
            "resources/read",
            json!({"uri":"velo://reference/contracts"}),
        )
        .await;
    assert!(
        schema["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("export type TestDefinition")
    );
    let denied = session
        .request(8, "resources/read", json!({"uri":"file:///PRIVATE_PATH"}))
        .await;
    assert!(denied.get("error").is_some());
    assert!(!denied.to_string().contains("PRIVATE_PATH"));
    for (index, name) in [
        "start_run",
        "preview_request",
        "set_ai_key",
        "propose_definition",
    ]
    .iter()
    .enumerate()
    {
        let denied = session.tool(9 + index as u64, name, json!({})).await;
        assert!(denied.get("error").is_some());
    }
    let arguments = session
        .tool(14, "new_draft", json!({"PRIVATE_FIELD":"PRIVATE_VALUE"}))
        .await;
    assert_eq!(arguments["error"]["code"], -32602);
    assert!(!arguments.to_string().contains("PRIVATE_VALUE"));
    session.close().await;
}

#[tokio::test]
async fn proposals_are_opt_in_immutable_and_importable_without_execution() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("original.velo.test.json");
    std::fs::write(&original, "DO_NOT_OVERWRITE").unwrap();
    let mut session = Session::start(&[Path::new("--draft-directory"), directory.path()]).await;
    let tools = session.request(2, "tools/list", json!({})).await;
    let proposal = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "propose_definition")
        .unwrap();
    assert_eq!(proposal["annotations"]["readOnlyHint"], false);
    assert_eq!(proposal["annotations"]["destructiveHint"], false);
    let mut draft = definition();
    draft.config.budget.max_vus = 1;
    let source = serde_json::to_string(&draft).unwrap();
    let first = session
        .tool(3, "propose_definition", json!({"source":source}))
        .await;
    let first = &first["result"]["structuredContent"];
    assert_eq!(first["review_required"], true);
    let path = Path::new(first["path"].as_str().unwrap());
    assert_eq!(
        path.parent(),
        Some(directory.path().canonicalize().unwrap().as_path())
    );
    let saved = std::fs::read_to_string(path).unwrap();
    let parsed = TestDefinition::parse(&saved).unwrap();
    assert_eq!(parsed.config.url, draft.config.url);
    assert_eq!(parsed.config.budget.max_vus, 1);
    let second = session
        .tool(4, "propose_definition", json!({"source":source}))
        .await;
    assert_ne!(second["result"]["structuredContent"]["id"], first["id"]);
    assert_eq!(std::fs::read_to_string(path).unwrap(), saved);
    assert_eq!(
        std::fs::read_to_string(original).unwrap(),
        "DO_NOT_OVERWRITE"
    );
    session.close().await;
}

fn report() -> ComparisonReport {
    ComparisonReport {
        format_version: 1,
        baseline_id: "PRIVATE_BASELINE".into(),
        baseline: None,
        candidate: HistoryEntry {
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
            definition_identity: None,
            configuration: json!({"url":"PRIVATE_URL"}),
            snapshot: json!({"metrics":{"request_rate":42,"diagnostics":{"samples":[{"body":"PRIVATE_BODY"}]}},"measurement":{"complete":false}}),
            points: vec![],
        },
        configuration_reviewed: false,
        differences: vec![],
        limitations: vec!["PRIVATE_LIMITATION".into()],
        deltas: vec![],
        regressions: vec![],
        verdict: RegressionVerdict::Unevaluable,
    }
}

#[tokio::test]
async fn only_startup_selected_frozen_redacted_evidence_is_exposed() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("PRIVATE_FILENAME.json");
    std::fs::write(&path, serde_json::to_vec(&report()).unwrap()).unwrap();
    let mut session = Session::start(&[Path::new("--evidence"), &path]).await;
    std::fs::write(&path, "CHANGED_AFTER_SELECTION").unwrap();
    let listed = session.tool(2, "list_evidence", json!({})).await;
    assert_eq!(
        listed["result"]["structuredContent"]["reports"][0]["id"],
        "report-1"
    );
    let read = session
        .tool(3, "read_evidence", json!({"id":"report-1"}))
        .await;
    let content = &read["result"]["structuredContent"];
    assert_eq!(
        content["evidence"]["candidate"]["metrics"]["request_rate"],
        42
    );
    assert_eq!(content["evidence"]["candidate"]["incomplete"], true);
    assert_eq!(content["evidence"]["comparison_limited"], true);
    assert!(!read.to_string().contains("PRIVATE_"));
    assert!(!read.to_string().contains("CHANGED_AFTER_SELECTION"));
    let resource = session
        .request(
            4,
            "resources/read",
            json!({"uri":"velo://evidence/report-1"}),
        )
        .await;
    let resource: Value =
        serde_json::from_str(resource["result"]["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(&resource, content);
    let denied = session
        .tool(5, "read_evidence", json!({"id":"../../PRIVATE_PATH"}))
        .await;
    assert_eq!(denied["result"]["isError"], true);
    assert!(!denied.to_string().contains("PRIVATE_PATH"));
    session.close().await;
}

#[tokio::test]
async fn offline_import_and_protocol_errors_do_not_execute_or_expose_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("never-created");
    let mut session = Session::start(&[]).await;
    let imported = session
        .tool(
            2,
            "import_request",
            json!({"format":"curl","source":"curl https://example.com/health"}),
        )
        .await;
    assert_eq!(
        imported["result"]["structuredContent"]["candidates"][0]["definition"]["config"]["method"],
        "GET"
    );
    let rejected = session.tool(3,"import_request",json!({"format":"curl","source":format!("curl https://example.com; touch {}",marker.display())})).await;
    assert_eq!(rejected["result"]["isError"], true);
    assert!(!marker.exists());
    let rejected = session
        .tool(
            4,
            "validate_definition",
            json!({"source":"PRIVATE_MALFORMED_JSON"}),
        )
        .await;
    assert_eq!(
        rejected["result"]["structuredContent"]["schema_valid"],
        false
    );
    assert!(!rejected.to_string().contains("PRIVATE_MALFORMED_JSON"));
    session
        .input
        .write_all(b"{INVALID_PRIVATE_JSON}\n")
        .await
        .unwrap();
    session.input.flush().await.unwrap();
    let ping = session.request(5, "ping", json!({})).await;
    assert!(ping.get("result").is_some());
    session.close().await;
}

#[tokio::test]
async fn oversized_frames_are_bounded_and_session_recovers() {
    let mut session = Session::start(&[]).await;
    session
        .input
        .write_all(&vec![b' '; 1_048_577])
        .await
        .unwrap();
    session.input.write_all(b"\n").await.unwrap();
    session.input.flush().await.unwrap();
    assert!(
        session
            .request(2, "ping", json!({}))
            .await
            .get("result")
            .is_some()
    );
    session.close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn termination_closes_an_idle_session_with_stdin_still_open() {
    let session = Session::start(&[]).await;
    let process_id = session.child.id().unwrap().to_string();
    let signal = Command::new("/bin/kill")
        .args(["-TERM", &process_id])
        .status()
        .await
        .unwrap();
    assert!(signal.success());
    let output = tokio::time::timeout(Duration::from_secs(5), session.child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    drop(session.input);
}

#[tokio::test]
async fn startup_failures_omit_paths_and_argument_values() {
    let output = Command::new(env!("CARGO_BIN_EXE_velo-mcp"))
        .args(["--evidence", "/PRIVATE_MISSING_REPORT_PATH"])
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("MCP_EVIDENCE_UNAVAILABLE"));
    assert!(!stderr.contains("PRIVATE_MISSING_REPORT_PATH"));
    let output = Command::new(env!("CARGO_BIN_EXE_velo-mcp"))
        .args(["--PRIVATE_UNKNOWN_OPTION", "PRIVATE_ARGUMENT"])
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("MCP_INVALID_ARGUMENTS"));
    assert!(!stderr.contains("PRIVATE_"));
}

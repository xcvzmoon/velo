use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::Mutex;
use velo_engine::{RunConfig, RunConfirmation, RunError, RunManager, RunSnapshot};

#[derive(Default)]
pub(crate) struct DesktopRuns {
    pub manager: RunManager,
    pub closing: AtomicBool,
    pub exit_ready: AtomicBool,
    pub admission: Mutex<()>,
}

#[tauri::command]
pub(crate) fn prepare_run(config: RunConfig) -> Result<RunConfirmation, RunError> {
    RunManager::prepare(config)
}

#[tauri::command]
pub(crate) async fn start_run(
    state: tauri::State<'_, Arc<DesktopRuns>>,
    config: RunConfig,
    authorized: bool,
) -> Result<RunSnapshot, RunError> {
    let _admission = state.admission.lock().await;
    if state.closing.load(Ordering::Acquire) {
        return Err(RunError {
            code: "APP_EXITING".into(),
            message: "The application is shutting down.".into(),
            field: None,
        });
    }
    let snapshot = state.manager.start(config, authorized).await?;
    tracing::info!(
        run_id = snapshot.run_id,
        vus = snapshot.confirmation.vus,
        "Run accepted"
    );
    Ok(snapshot)
}

#[tauri::command]
pub(crate) async fn stop_run(
    state: tauri::State<'_, Arc<DesktopRuns>>,
    run_id: String,
) -> Result<RunSnapshot, RunError> {
    state.manager.stop(&run_id).await
}

#[tauri::command]
pub(crate) async fn current_run(
    state: tauri::State<'_, Arc<DesktopRuns>>,
) -> Result<Option<RunSnapshot>, RunError> {
    Ok(state.manager.current().await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use tauri::{
        WebviewWindow,
        ipc::{CallbackFn, InvokeBody},
        test::{MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets},
        webview::InvokeRequest,
    };

    fn invoke(
        window: &WebviewWindow<MockRuntime>,
        command: &str,
        body: Value,
    ) -> Result<Value, Value> {
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

    #[test]
    fn native_commands_validate_serialize_and_preserve_run_ownership() {
        let state = Arc::new(DesktopRuns::default());
        let app = mock_builder()
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![
                prepare_run,
                start_run,
                stop_run,
                current_run
            ])
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        assert!(invoke(&window, "current_run", json!({})).unwrap().is_null());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let config = RunConfig {
            url: format!(
                "http://{}/private?token=secret",
                listener.local_addr().unwrap()
            ),
            duration_ms: 30_000,
            grace_ms: 0,
            ..RunConfig::default()
        };
        let confirmation = invoke(&window, "prepare_run", json!({ "config": config })).unwrap();
        assert!(!confirmation.to_string().contains("secret"));
        assert_eq!(confirmation["vus"], 1);
        let unauthorized = invoke(
            &window,
            "start_run",
            json!({ "config": config, "authorized": false }),
        )
        .unwrap_err();
        assert_eq!(unauthorized["code"], "AUTHORIZATION_REQUIRED");
        let mut invalid = config.clone();
        invalid.timeout_ms = 0;
        let error = invoke(
            &window,
            "start_run",
            json!({ "config": invalid, "authorized": true }),
        )
        .unwrap_err();
        assert_eq!(error["code"], "INVALID_RUN_CONFIG");
        assert_eq!(error["field"], "timeout_ms");
        assert!(!error.to_string().contains("secret"));

        let accepted = invoke(
            &window,
            "start_run",
            json!({ "config": config, "authorized": true }),
        )
        .unwrap();
        assert!(accepted["run_id"].is_string());
        assert!(accepted["metrics"]["counters"]["started"].is_string());
        assert_eq!(
            invoke(
                &window,
                "start_run",
                json!({ "config": config, "authorized": true })
            )
            .unwrap_err()["code"],
            "RUN_ALREADY_ACTIVE"
        );
        assert_eq!(
            invoke(&window, "stop_run", json!({ "runId": "stale" })).unwrap_err()["code"],
            "RUN_NOT_FOUND"
        );
        invoke(&window, "stop_run", json!({ "runId": accepted["run_id"] })).unwrap();
        tauri::async_runtime::block_on(state.manager.shutdown());
        let summary = invoke(&window, "current_run", json!({})).unwrap();
        assert_eq!(summary["state"], "cancelled");
        assert_eq!(summary["active_vus"], 0);
        assert_eq!(summary["metrics"]["counters"]["in_flight"], "0");
        assert!(!summary.to_string().contains("secret"));
        state.closing.store(true, Ordering::Release);
        assert_eq!(
            invoke(
                &window,
                "start_run",
                json!({ "config": config, "authorized": true })
            )
            .unwrap_err()["code"],
            "APP_EXITING"
        );
    }
}

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tauri::{Emitter, Manager, async_runtime::JoinHandle};
use tokio::{
    sync::Mutex,
    time::{Duration, MissedTickBehavior},
};
use velo_engine::{RunConfig, RunConfirmation, RunError, RunManager, RunSnapshot, RunState};

#[derive(Default)]
pub(crate) struct DesktopRuns {
    pub manager: RunManager,
    pub preparation: std::sync::Mutex<Option<Arc<AtomicBool>>>,
    pub closing: AtomicBool,
    pub exit_ready: AtomicBool,
    pub admission: Mutex<()>,
    pub preview_exit: tokio::sync::Notify,
    pub monitor: Mutex<Option<JoinHandle<()>>>,
}

#[tauri::command]
pub(crate) async fn prepare_run(config: RunConfig) -> Result<RunConfirmation, RunError> {
    tauri::async_runtime::spawn_blocking(move || RunManager::prepare(config))
        .await
        .map_err(|_| RunError {
            code: "RUN_PREPARATION_FAILED".into(),
            message: "Request preparation could not complete.".into(),
            field: None,
        })?
}

fn step_identity(step: &velo_engine::ScenarioStep) -> serde_json::Value {
    serde_json::json!({ "id": step.id, "method": step.request.method, "think_ms": step.think_ms, "on_failure": step.on_failure })
}

#[tauri::command]
pub(crate) async fn start_run<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, Arc<DesktopRuns>>,
    config: RunConfig,
    authorized: bool,
    update_interval_ms: Option<u32>,
) -> Result<RunSnapshot, RunError> {
    let _admission = state.admission.lock().await;
    if state.closing.load(Ordering::Acquire) {
        return Err(RunError {
            code: "APP_EXITING".into(),
            message: "The application is shutting down.".into(),
            field: None,
        });
    }
    let interval_ms = update_interval_ms.unwrap_or(1_000);
    if !(250..=10_000).contains(&interval_ms) {
        return Err(RunError {
            code: "INVALID_UPDATE_INTERVAL".into(),
            message: "Update interval must be between 250 and 10,000 milliseconds.".into(),
            field: Some("update_interval_ms".into()),
        });
    }
    let mut monitor = state.monitor.lock().await;
    if state.manager.current().await.is_none_or(|snapshot| {
        matches!(
            snapshot.state,
            RunState::Completed | RunState::Cancelled | RunState::Failed | RunState::Aborted
        )
    }) && let Some(previous) = monitor.take()
    {
        let _ = previous.await;
    }
    let method = format!("{:?}", config.method);
    let scenario_identity = config.scenarios.as_ref().map(|plan| {
        serde_json::json!({
            "seed": plan.seed,
            "setup": plan.setup.iter().map(step_identity).collect::<Vec<_>>(),
            "cleanup": plan.cleanup.iter().map(step_identity).collect::<Vec<_>>(),
            "scenarios": plan.scenarios.iter().map(|scenario| serde_json::json!({
                "id": scenario.id, "weight": scenario.weight,
                "steps": scenario.steps.iter().map(step_identity).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    });
    let configuration = serde_json::json!({
        "scenarios": scenario_identity,
        "method": config.method, "protocol": config.protocol, "max_redirects": config.max_redirects,
        "timeout_ms": config.timeout_ms, "pacing_ms": config.pacing_ms, "grace_ms": config.grace_ms,
        "workload": config.workload, "warmup_ms": config.warmup_ms, "thresholds": config.thresholds,
        "abort_policies": config.abort_policies, "inspection": config.inspection,
        "assertion_ids": config.assertions.iter().map(|assertion| &assertion.id).collect::<Vec<_>>(),
    });
    let snapshot = state.manager.start(config, authorized).await?;
    let history_id = uuid::Uuid::new_v4().to_string();
    let mut incomplete = if let Some(history) = app.try_state::<crate::history::History>() {
        !history
            .begin_with_context(history_id.clone(), method, snapshot.clone(), configuration)
            .await
    } else {
        true
    };
    let runs = state.inner().clone();
    let run_id = snapshot.run_id.clone();
    *monitor = Some(tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms.into()));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let Some(update) = runs.manager.sample(&run_id).await else {
                break;
            };
            let terminal = matches!(
                update.snapshot.state,
                RunState::Completed | RunState::Cancelled | RunState::Failed | RunState::Aborted
            );
            if let Some(history) = app.try_state::<crate::history::History>() {
                if terminal {
                    history
                        .finish(history_id.clone(), update.clone(), incomplete)
                        .await;
                } else if !history.record(&history_id, update.clone(), incomplete) {
                    incomplete = true;
                }
            }
            if app.emit_to("main", "run-update", &update).is_err() {
                tracing::warn!(run_id, "Aggregate update delivery failed");
            }
            if terminal {
                break;
            }
        }
    }));
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
        Listener, WebviewWindow,
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
        let directory = tempfile::tempdir().unwrap();
        let app = mock_builder()
            .manage(crate::history::History::new(Some(
                directory.path().join("history.sqlite3"),
            )))
            .manage(state.clone())
            .invoke_handler(tauri::generate_handler![
                crate::reports::pinned_baseline,
                crate::reports::pin_baseline,
                crate::reports::update_run_metadata,
                crate::reports::load_comparison_settings,
                crate::reports::save_comparison,
                crate::history::list_history,
                crate::history::load_history,
                crate::history::save_configuration,
                crate::history::list_configurations,
                crate::history::load_configuration,
                crate::history::delete_configuration,
                crate::authoring::preview_request,
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
            workload: velo_engine::Workload::Constant {
                vus: 1,
                duration_ms: 30_000,
            },
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
        let definition = velo_engine::TestDefinition {
            config: config.clone(),
            ..Default::default()
        };
        let unauthorized_preview = invoke(
            &window,
            "preview_request",
            json!({ "definition": definition, "authorized": false }),
        )
        .unwrap_err();
        assert_eq!(unauthorized_preview["code"], "AUTHORIZATION_REQUIRED");
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

        let (updates, received) = std::sync::mpsc::channel();
        window.listen("run-update", move |event| {
            updates
                .send(serde_json::from_str::<Value>(event.payload()).unwrap())
                .unwrap();
        });
        let invalid_interval = invoke(
            &window,
            "start_run",
            json!({ "config": config, "authorized": true, "updateIntervalMs": 1 }),
        )
        .unwrap_err();
        assert_eq!(invalid_interval["code"], "INVALID_UPDATE_INTERVAL");
        assert!(invoke(&window, "current_run", json!({})).unwrap().is_null());
        let accepted = invoke(
            &window,
            "start_run",
            json!({ "config": config, "authorized": true, "updateIntervalMs": 250 }),
        )
        .unwrap();
        let blocked_preview = invoke(
            &window,
            "preview_request",
            json!({ "definition": definition, "authorized": true }),
        )
        .unwrap_err();
        assert_eq!(blocked_preview["code"], "RUN_ALREADY_ACTIVE");
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
        let mut ramp = config.clone();
        ramp.workload = velo_engine::Workload::Ramping {
            start_vus: 0,
            stages: vec![velo_engine::RampStage {
                duration_ms: 1_000,
                target_vus: 2,
            }],
        };
        let confirmation = invoke(&window, "prepare_run", json!({ "config": ramp })).unwrap();
        assert_eq!(confirmation["vus"], 2);
        assert_eq!(confirmation["workload"]["kind"], "ramping");
        assert_eq!(confirmation["workload"]["stages"][0]["target_vus"], 2);
        assert_eq!(summary["desired_vus"], 0);
        assert_eq!(summary["retiring_vus"], 0);
        let mut arrival = config.clone();
        arrival.pacing_ms = 0;
        arrival.workload = velo_engine::Workload::Arrival {
            rate: 50,
            capacity: 3,
            duration_ms: 1_000,
        };
        let confirmation = invoke(&window, "prepare_run", json!({ "config": arrival })).unwrap();
        assert_eq!(confirmation["vus"], 3);
        assert_eq!(confirmation["workload"]["kind"], "arrival");
        assert_eq!(confirmation["workload"]["rate"], 50);
        assert_eq!(confirmation["workload"]["capacity"], 3);
        let mut sequence = 0_u64;
        loop {
            let update = received.recv_timeout(Duration::from_secs(2)).unwrap();
            let next = update["sequence"].as_str().unwrap().parse::<u64>().unwrap();
            assert!(next > sequence);
            sequence = next;
            assert_eq!(update["snapshot"]["run_id"], accepted["run_id"]);
            assert!(update["interval_requests"].is_string());
            assert!(!update.to_string().contains("secret"));
            if update["snapshot"]["state"] == "cancelled" {
                assert_eq!(update["snapshot"]["metrics"]["counters"]["in_flight"], "0");
                break;
            }
        }
        let history = invoke(&window, "list_history", json!({})).unwrap();
        assert_eq!(history.as_array().unwrap().len(), 1);
        assert_eq!(history[0]["snapshot"]["state"], "cancelled");
        let stored = invoke(&window, "load_history", json!({ "id": history[0]["id"] })).unwrap();
        assert!(!stored["points"].as_array().unwrap().is_empty());
        assert!(!stored.to_string().contains("secret"));
        assert!(!stored.to_string().contains("private"));
        assert!(
            stored["definition_identity"]
                .as_str()
                .unwrap()
                .starts_with("safe-v1:")
        );
        assert_eq!(stored["configuration"]["timeout_ms"], 30000);
        assert!(
            invoke(&window, "pinned_baseline", json!({}))
                .unwrap()
                .is_null()
        );
        invoke(&window, "pin_baseline", json!({"id": history[0]["id"]})).unwrap();
        assert_eq!(
            invoke(&window, "pinned_baseline", json!({})).unwrap(),
            history[0]["id"]
        );
        let measured = stored["snapshot"].clone();
        let metadata =
            json!({"git_sha":"abc1234", "release":"v1.0", "tags":["release"], "notes":"reviewed"});
        invoke(
            &window,
            "update_run_metadata",
            json!({"id":history[0]["id"], "metadata":metadata}),
        )
        .unwrap();
        let updated = invoke(&window, "load_history", json!({"id":history[0]["id"]})).unwrap();
        assert_eq!(updated["metadata"], metadata);
        assert_eq!(updated["snapshot"], measured);
        let mut invalid_metadata = metadata;
        invalid_metadata["git_sha"] = json!("invalid");
        assert_eq!(
            invoke(
                &window,
                "update_run_metadata",
                json!({"id":history[0]["id"], "metadata":invalid_metadata})
            )
            .unwrap_err()["code"],
            "INVALID_REPORT_SETTINGS"
        );
        let settings = json!({"baseline_id":history[0]["id"], "configuration_reviewed":true, "budgets":[{"metric":"response_p95_ms", "limit":10.0, "percentage":true}]});
        let report = invoke(
            &window,
            "save_comparison",
            json!({"id":history[0]["id"], "settings":settings}),
        )
        .unwrap();
        assert_eq!(report["format_version"], 1);
        assert_eq!(report["verdict"], "unevaluable");
        assert_eq!(report["regressions"][0]["verdict"], "unevaluable");
        assert_eq!(
            invoke(
                &window,
                "load_comparison_settings",
                json!({"id":history[0]["id"]})
            )
            .unwrap(),
            settings
        );
        invoke(&window, "pin_baseline", json!({"id":null})).unwrap();
        assert!(
            invoke(&window, "pinned_baseline", json!({}))
                .unwrap()
                .is_null()
        );

        invoke(
            &window,
            "save_configuration",
            json!({ "name": "Example", "config": config, "remember": false }),
        )
        .unwrap();
        let saved = invoke(&window, "list_configurations", json!({})).unwrap();
        assert_eq!(saved.as_array().unwrap().len(), 1);
        let loaded = invoke(
            &window,
            "load_configuration",
            json!({ "id": saved[0]["id"] }),
        )
        .unwrap();
        assert!(!loaded.to_string().contains("secret"));
        assert!(!loaded.to_string().contains("private"));
        invoke(
            &window,
            "delete_configuration",
            json!({ "id": saved[0]["id"] }),
        )
        .unwrap();
        assert!(
            invoke(&window, "list_configurations", json!({}))
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
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

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{Router, extract::State, http::StatusCode, routing::get};
use tokio::{
    net::TcpListener,
    task::JoinHandle,
    time::{sleep, timeout},
};
use velo_engine::{
    AbortPolicy, RunConfig, RunManager, RunSnapshot, RunState, Threshold, ThresholdMetric,
    ThresholdOperator, Workload,
};

struct Server {
    url: String,
    task: JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn server(warmup_failure: bool) -> Server {
    let state = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route(
            "/",
            get(move |State(count): State<Arc<AtomicUsize>>| async move {
                let first = count.fetch_add(1, Ordering::SeqCst) == 0;
                sleep(Duration::from_millis(if warmup_failure && first {
                    220
                } else {
                    20
                }))
                .await;
                if !warmup_failure || first {
                    StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    StatusCode::OK
                }
            }),
        )
        .with_state(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Server { url, task }
}

fn rule() -> Threshold {
    Threshold {
        metric: ThresholdMetric::HttpFailurePercent,
        operator: ThresholdOperator::LessThan,
        limit: 50.0,
    }
}

async fn finish(manager: &RunManager) -> RunSnapshot {
    timeout(Duration::from_secs(4), async {
        loop {
            let snapshot = manager.current().await.unwrap();
            if !matches!(
                snapshot.state,
                RunState::Starting | RunState::Running | RunState::Stopping
            ) {
                return snapshot;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

fn reconcile(snapshot: &RunSnapshot) {
    for metrics in [&snapshot.metrics, &snapshot.evaluated] {
        let counters = &metrics.counters;
        assert_eq!(counters.in_flight, 0);
        assert_eq!(
            counters.started,
            counters.responses
                + counters.transport_failures
                + counters.timeouts
                + counters.cancelled
        );
    }
    assert_eq!(snapshot.active_vus, 0);
}

#[tokio::test]
async fn late_warmup_response_is_visible_but_excluded_from_final_thresholds() {
    let server = server(true).await;
    let manager = RunManager::default();
    manager
        .start(
            RunConfig {
                url: server.url.clone(),
                warmup_ms: 100,
                workload: Workload::Constant {
                    vus: 1,
                    duration_ms: 450,
                },
                thresholds: vec![rule()],
                ..RunConfig::default()
            },
            true,
        )
        .await
        .unwrap();
    let snapshot = finish(&manager).await;
    assert_eq!(snapshot.state, RunState::Completed);
    assert!(snapshot.measurement.complete);
    assert_eq!(snapshot.metrics.counters.status_failures, 1);
    assert_eq!(snapshot.evaluated.counters.status_failures, 0);
    assert!(snapshot.evaluated.counters.successes > 0);
    assert!(matches!(
        snapshot.thresholds[0].status,
        velo_engine::ThresholdStatus::Passed
    ));
    assert_eq!(
        snapshot.metrics.counters.started,
        snapshot.evaluated.counters.started + 1
    );
    reconcile(&snapshot);
    manager.shutdown().await;
}

#[tokio::test]
async fn sustained_failure_aborts_vu_arrival_and_scenario_admission_with_partial_evidence() {
    let server = server(false).await;
    for kind in ["constant", "arrival", "scenario"] {
        let manager = RunManager::default();
        let mut config = RunConfig {
            url: server.url.clone(),
            warmup_ms: 200,
            workload: if kind == "arrival" {
                Workload::Arrival {
                    rate: 50,
                    capacity: 2,
                    duration_ms: 2_000,
                }
            } else {
                Workload::Constant {
                    vus: 1,
                    duration_ms: 2_000,
                }
            },
            abort_policies: vec![AbortPolicy {
                rule: rule(),
                window_ms: 100,
                min_samples: 1,
                sustained_ms: 200,
            }],
            thresholds: vec![rule()],
            ..RunConfig::default()
        };
        if kind == "scenario" {
            config.scenarios = Some(velo_engine::ScenarioPlan {
                setup: vec![],
                cleanup: vec![],
                seed: 1,
                scenarios: vec![velo_engine::Scenario {
                    id: "journey".into(),
                    weight: 1,
                    steps: vec![velo_engine::ScenarioStep {
                        id: "request".into(),
                        request: velo_engine::ScenarioRequest {
                            url: server.url.clone(),
                            method: velo_engine::RequestMethod::GET,
                            headers: vec![],
                            query: vec![],
                            body: velo_engine::RequestBody::None,
                            auth: velo_engine::RequestAuth::None,
                            assertions: vec![],
                        },
                        extractions: vec![],
                        think_ms: 0,
                        on_failure: velo_engine::StepFailure::Continue,
                    }],
                }],
            });
        }
        manager.start(config, true).await.unwrap();
        let snapshot = finish(&manager).await;
        assert_eq!(snapshot.state, RunState::Aborted, "{kind}");
        assert!(!snapshot.measurement.complete);
        let evidence = snapshot.automatic_stop.as_ref().unwrap();
        assert!(evidence.at_us >= 500_000);
        assert!(evidence.at_us < 2_000_000);
        assert_eq!(evidence.observed, Some(100.0));
        assert!(evidence.breached_us >= 200_000);
        assert!(evidence.samples > 0);
        assert!(matches!(
            snapshot.thresholds[0].status,
            velo_engine::ThresholdStatus::Failed
        ));
        assert!(snapshot.failure_code.is_none());
        reconcile(&snapshot);
        manager.shutdown().await;
    }
}

#[tokio::test]
async fn user_stop_during_warmup_has_empty_evaluation_and_insufficient_threshold_data() {
    let server = server(false).await;
    let manager = RunManager::default();
    let accepted = manager
        .start(
            RunConfig {
                url: server.url.clone(),
                warmup_ms: 1_000,
                workload: Workload::Constant {
                    vus: 1,
                    duration_ms: 2_000,
                },
                thresholds: vec![rule()],
                abort_policies: vec![AbortPolicy {
                    rule: rule(),
                    window_ms: 100,
                    min_samples: 1,
                    sustained_ms: 100,
                }],
                ..RunConfig::default()
            },
            true,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(100)).await;
    manager.stop(&accepted.run_id).await.unwrap();
    let snapshot = finish(&manager).await;
    assert_eq!(snapshot.state, RunState::Cancelled);
    assert!(snapshot.automatic_stop.is_none());
    assert_eq!(snapshot.evaluated.counters.started, 0);
    assert_eq!(snapshot.measurement.evaluated_us, 0);
    assert!(matches!(
        snapshot.thresholds[0].status,
        velo_engine::ThresholdStatus::InsufficientData
    ));
    reconcile(&snapshot);
    manager.shutdown().await;
}

#[test]
fn invalid_measurement_and_policy_input_is_rejected_before_start() {
    for warmup_ms in [100, 101] {
        let result = RunManager::prepare(RunConfig {
            url: "http://127.0.0.1/".into(),
            warmup_ms,
            workload: Workload::Constant {
                vus: 1,
                duration_ms: 100,
            },
            ..RunConfig::default()
        });
        assert_eq!(result.unwrap_err().code, "INVALID_MEASUREMENT_WINDOW");
    }
    for (window_ms, min_samples, sustained_ms) in
        [(0, 1, 100), (101, 1, 100), (100, 0, 100), (100, 1, 60_100)]
    {
        let result = RunManager::prepare(RunConfig {
            url: "http://127.0.0.1/".into(),
            abort_policies: vec![AbortPolicy {
                rule: rule(),
                window_ms,
                min_samples,
                sustained_ms,
            }],
            ..RunConfig::default()
        });
        assert_eq!(result.unwrap_err().code, "INVALID_ABORT_POLICY");
    }
}

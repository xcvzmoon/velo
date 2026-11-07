use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router,
    extract::{Path, State},
    routing::post,
};
use velo_engine::{
    DataAllocation, DataDefinition, DataExhaustion, DataLifetime, DataSource, RequestBody,
    RequestMethod, RunConfig, RunManager, RunSnapshot, RunState, TestDefinition, Workload,
};

type Received = Arc<Mutex<Vec<(String, String)>>>;

async fn receive(
    State(received): State<Received>,
    Path(id): Path<String>,
    body: String,
) -> &'static str {
    received.lock().unwrap().push((id, body));
    tokio::time::sleep(Duration::from_millis(2)).await;
    "ok"
}

fn definition(
    origin: &str,
    lifetime: DataLifetime,
    allocation: DataAllocation,
    exhaustion: DataExhaustion,
) -> TestDefinition {
    TestDefinition {
        config: RunConfig {
            url: format!("{origin}/{{{{id}}}}"),
            method: RequestMethod::POST,
            body: RequestBody::Json(r#"{"id":"{{id}}","value":"{{value}}"}"#.into()),
            workload: Workload::Constant {
                vus: 8,
                duration_ms: 100,
            },
            grace_ms: 1000,
            pacing_ms: 2,
            ..Default::default()
        },
        dataset: Some(DataDefinition {
            lifetime,
            allocation,
            exhaustion,
            seed: 42,
            source: DataSource::Json {
                mapping: BTreeMap::from([
                    ("id".into(), "id".into()),
                    ("value".into(), "value".into()),
                ]),
            },
        }),
        ..Default::default()
    }
}

fn rows(count: usize) -> Vec<BTreeMap<String, String>> {
    (0..count)
        .map(|index| {
            BTreeMap::from([
                ("id".into(), index.to_string()),
                ("value".into(), "PRIVATE_ROW_\"é{{secret:never}}".into()),
            ])
        })
        .collect()
}

async fn terminal(manager: &RunManager) -> RunSnapshot {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = manager.current().await.unwrap();
            if matches!(
                snapshot.state,
                RunState::Completed | RunState::Failed | RunState::Cancelled
            ) {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap()
}

async fn execute(
    lifetime: DataLifetime,
    allocation: DataAllocation,
    exhaustion: DataExhaustion,
    count: usize,
    workers: u32,
) -> (RunSnapshot, Vec<(String, String)>) {
    let received = Received::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route("/{id}", post(receive))
        .with_state(received.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let mut definition = definition(&origin, lifetime, allocation, exhaustion);
    definition.config.workload = Workload::Constant {
        vus: workers,
        duration_ms: if lifetime == DataLifetime::Iteration && exhaustion != DataExhaustion::Repeat
        {
            1000
        } else {
            100
        },
    };
    let config = definition
        .resolve_data(
            None,
            &rows(count),
            || false,
            |_| panic!("Dataset values must remain literals"),
        )
        .unwrap();
    let manager = RunManager::default();
    manager.start(config, true).await.unwrap();
    let snapshot = terminal(&manager).await;
    manager.shutdown().await;
    server.abort();
    let _ = server.await;
    let captured = received.lock().unwrap().clone();
    (snapshot, captured)
}

#[tokio::test]
async fn unique_concurrent_allocation_exhaustion_and_history_are_safe() {
    for exhaustion in [DataExhaustion::Stop, DataExhaustion::Fail] {
        let (snapshot, captured) = execute(
            DataLifetime::Iteration,
            DataAllocation::Unique,
            exhaustion,
            23,
            8,
        )
        .await;
        assert_eq!(captured.len(), 23);
        let indexes: std::collections::BTreeSet<_> = captured.iter().map(|(id, _)| id).collect();
        assert_eq!(indexes.len(), 23);
        for (id, body) in &captured {
            let body: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(body["id"], id.as_str());
            assert_eq!(body["value"], "PRIVATE_ROW_\"é{{secret:never}}");
        }
        assert_eq!(snapshot.metrics.counters.started, 23);
        let data = snapshot.dataset.as_ref().unwrap();
        assert!(data.exhausted);
        assert_eq!(data.allocated, "23");
        assert_eq!(snapshot.active_vus, 0);
        assert!(
            !serde_json::to_string(&snapshot)
                .unwrap()
                .contains("PRIVATE_ROW")
        );
        match exhaustion {
            DataExhaustion::Fail => {
                assert_eq!(snapshot.state, RunState::Failed);
                assert_eq!(snapshot.failure_code.as_deref(), Some("DATASET_EXHAUSTED"));
            }
            _ => assert_eq!(snapshot.state, RunState::Completed),
        }
    }
}

#[tokio::test]
async fn run_and_user_lifetimes_reuse_records_and_repeat_is_counted() {
    let (run, captured) = execute(
        DataLifetime::Run,
        DataAllocation::Unique,
        DataExhaustion::Stop,
        1,
        8,
    )
    .await;
    assert!(captured.len() > 1);
    assert!(captured.iter().all(|(id, _)| id == "0"));
    assert_eq!(run.dataset.unwrap().allocated, "1");
    let (user, captured) = execute(
        DataLifetime::User,
        DataAllocation::Unique,
        DataExhaustion::Stop,
        8,
        8,
    )
    .await;
    assert!(captured.len() > 8);
    assert_eq!(user.dataset.unwrap().allocated, "8");
    let (repeat, captured) = execute(
        DataLifetime::Iteration,
        DataAllocation::Sequential,
        DataExhaustion::Repeat,
        2,
        8,
    )
    .await;
    let data = repeat.dataset.unwrap();
    assert!(captured.len() > 2);
    assert!(data.cycles.parse::<u64>().unwrap() > 0);
    assert!(!data.exhausted);
}

#[test]
fn preparation_rejects_invalid_records_and_unsafe_policies_before_traffic() {
    let mut definition = definition(
        "http://127.0.0.1:1",
        DataLifetime::Iteration,
        DataAllocation::Unique,
        DataExhaustion::Repeat,
    );
    assert_eq!(
        definition
            .resolve_data(None, &rows(1), || false, |_| unreachable!())
            .unwrap_err()
            .code,
        "INVALID_DATASET"
    );
    definition.dataset.as_mut().unwrap().exhaustion = DataExhaustion::Stop;
    assert_eq!(
        definition
            .resolve_data(None, &[], || false, |_| unreachable!())
            .unwrap_err()
            .code,
        "INVALID_DATASET"
    );
    let mut missing = rows(1);
    missing[0].remove("id");
    assert_eq!(
        definition
            .resolve_data(None, &missing, || false, |_| unreachable!())
            .unwrap_err()
            .code,
        "INVALID_DATASET"
    );
    definition.config.budget.max_request_bytes = 256;
    assert_eq!(
        definition
            .resolve_data(None, &rows(100), || false, |_| unreachable!())
            .unwrap_err()
            .code,
        "DATASET_BUDGET_EXCEEDED"
    );
}

#[test]
fn portable_definitions_keep_recipe_and_reject_embedded_prepared_requests() {
    let definition = definition(
        "http://127.0.0.1:1",
        DataLifetime::Iteration,
        DataAllocation::Random,
        DataExhaustion::Stop,
    );
    let source = serde_json::to_string(&definition).unwrap();
    assert!(TestDefinition::parse(&source).is_ok());
    assert!(!source.contains("PRIVATE_ROW"));
    let config = definition
        .resolve_data(None, &rows(3), || false, |_| unreachable!())
        .unwrap();
    let mut definition = definition;
    definition.config = config;
    assert_eq!(
        TestDefinition::parse(&serde_json::to_string(&definition).unwrap())
            .unwrap_err()
            .code,
        "INVALID_TEST_DEFINITION"
    );
}

#[tokio::test]
async fn sequential_and_seeded_random_orders_are_reproducible_with_one_worker() {
    let (_, first) = execute(
        DataLifetime::Iteration,
        DataAllocation::Sequential,
        DataExhaustion::Stop,
        8,
        1,
    )
    .await;
    let sequential: Vec<_> = first.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(
        sequential,
        (0..8).map(|index| index.to_string()).collect::<Vec<_>>()
    );
    let (_, first) = execute(
        DataLifetime::Iteration,
        DataAllocation::Random,
        DataExhaustion::Stop,
        8,
        1,
    )
    .await;
    let (_, second) = execute(
        DataLifetime::Iteration,
        DataAllocation::Random,
        DataExhaustion::Stop,
        8,
        1,
    )
    .await;
    assert_eq!(first, second);
    assert_ne!(
        first.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
        sequential
    );
}

#[test]
fn cancellation_and_late_record_validation_fail_before_execution() {
    let definition = definition(
        "http://127.0.0.1:1",
        DataLifetime::Iteration,
        DataAllocation::Sequential,
        DataExhaustion::Stop,
    );
    let mut checks = 0;
    let error = definition
        .resolve_data(
            None,
            &rows(3),
            || {
                checks += 1;
                checks > 1
            },
            |_| unreachable!(),
        )
        .unwrap_err();
    assert_eq!(error.code, "PREPARATION_CANCELLED");
    let mut config = definition
        .resolve_data(None, &rows(3), || false, |_| unreachable!())
        .unwrap();
    config.data.as_mut().unwrap().requests[2]
        .headers
        .push(velo_engine::NameValue {
            name: "x-value".into(),
            value: "invalid\r\nheader".into(),
        });
    let error = RunManager::prepare(config).unwrap_err();
    assert_eq!(error.field.as_deref(), Some("dataset.records.2.headers"));
    let mut config = definition
        .resolve_data(None, &rows(3), || false, |_| unreachable!())
        .unwrap();
    config.data.as_mut().unwrap().requests[2].url = "https://example.com/".into();
    assert_eq!(
        RunManager::prepare(config).unwrap_err().code,
        "INVALID_DATASET"
    );
    let mut config = definition
        .resolve_data(None, &rows(3), || false, |_| unreachable!())
        .unwrap();
    config.workload = Workload::Arrival {
        rate: 10,
        capacity: 2,
        duration_ms: 1000,
    };
    config.pacing_ms = 0;
    config.data.as_mut().unwrap().definition.lifetime = DataLifetime::User;
    assert_eq!(
        RunManager::prepare(config).unwrap_err().code,
        "INVALID_DATASET"
    );
}

#[tokio::test]
async fn arrival_exhaustion_keeps_offered_slots_reconciled() {
    let mut definition = definition(
        "http://127.0.0.1:1",
        DataLifetime::Iteration,
        DataAllocation::Unique,
        DataExhaustion::Stop,
    );
    definition.config.workload = Workload::Arrival {
        rate: 100,
        capacity: 8,
        duration_ms: 1000,
    };
    definition.config.pacing_ms = 0;
    let config = definition
        .resolve_data(None, &rows(3), || false, |_| unreachable!())
        .unwrap();
    let manager = RunManager::default();
    manager.start(config, true).await.unwrap();
    let snapshot = terminal(&manager).await;
    assert_eq!(snapshot.arrival.dataset_drops, 1);
    assert_eq!(
        snapshot.arrival.offered,
        snapshot.metrics.counters.started
            + snapshot.arrival.capacity_drops
            + snapshot.arrival.missed_deadlines
            + snapshot.arrival.dataset_drops
    );
    assert_eq!(snapshot.dataset.unwrap().allocated, "3");
    manager.shutdown().await;
}

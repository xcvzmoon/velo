use std::sync::{Arc, Mutex, mpsc};

use tauri::Manager;
use velo_contracts::{HistoryEntry, SavedConfiguration};
use velo_engine::{RunConfig, RunError, RunSnapshot, RunUpdate};

use crate::{
    credentials,
    storage::{Store, failure, redacted},
};

type SharedStore = Arc<Mutex<Option<Store>>>;

pub(crate) struct History {
    store: SharedStore,
    admission: Arc<tokio::sync::Semaphore>,
    sender: mpsc::SyncSender<(String, RunUpdate, bool, Option<mpsc::SyncSender<()>>)>,
    status: Arc<Mutex<Option<String>>>,
}

impl History {
    pub(crate) fn new(path: Option<std::path::PathBuf>) -> Self {
        let opened = path.and_then(|path| Store::open(&path).ok());
        let status = Arc::new(Mutex::new(if opened.is_some() {
            None
        } else {
            Some("STORAGE_UNAVAILABLE".into())
        }));
        let store = Arc::new(Mutex::new(opened));
        let (sender, receiver) =
            mpsc::sync_channel::<(String, RunUpdate, bool, Option<mpsc::SyncSender<()>>)>(32);
        let worker_store = store.clone();
        let worker_status = status.clone();
        std::thread::spawn(move || {
            let mut failed_run: Option<String> = None;
            while let Ok((id, update, incomplete, acknowledged)) = receiver.recv() {
                let result = worker_store
                    .lock()
                    .map_err(|_| failure())
                    .and_then(|mut store| {
                        store.as_mut().ok_or_else(failure)?.record(
                            &id,
                            &update,
                            incomplete || failed_run.as_deref() == Some(&id),
                        )
                    });
                if result.is_err() {
                    failed_run = Some(id);
                }
                if result.is_err()
                    && let Ok(mut status) = worker_status.lock()
                {
                    *status = Some("HISTORY_WRITE_FAILED".into());
                }
                if let Some(acknowledged) = acknowledged {
                    let _ = acknowledged.send(());
                }
            }
        });
        Self {
            store,
            admission: Arc::new(tokio::sync::Semaphore::new(4)),
            sender,
            status,
        }
    }

    pub(crate) async fn begin(&self, id: String, method: String, snapshot: RunSnapshot) -> bool {
        let success = self
            .call(move |store| store.begin(&id, &method, &snapshot))
            .await
            .is_ok();
        if !success && let Ok(mut status) = self.status.lock() {
            *status = Some("HISTORY_WRITE_FAILED".into());
        }
        success
    }

    pub(crate) fn record(&self, id: &str, update: RunUpdate, incomplete: bool) -> bool {
        if self
            .sender
            .try_send((id.into(), update, incomplete, None))
            .is_err()
        {
            if let Ok(mut status) = self.status.lock() {
                *status = Some("HISTORY_BACKPRESSURE".into());
            }
            return false;
        }
        true
    }

    pub(crate) async fn finish(&self, id: String, update: RunUpdate, incomplete: bool) {
        let sender = self.sender.clone();
        let status = self.status.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let (acknowledged, received) = mpsc::sync_channel(1);
            if sender
                .send((id, update, incomplete, Some(acknowledged)))
                .is_err()
                && let Ok(mut status) = status.lock()
            {
                *status = Some("HISTORY_WRITE_FAILED".into());
            }
            let _ = received.recv();
        })
        .await;
    }

    async fn call<T: Send + 'static>(
        &self,
        action: impl FnOnce(&mut Store) -> Result<T, RunError> + Send + 'static,
    ) -> Result<T, RunError> {
        let permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| failure())?;
        let store = self.store.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            let mut guard = store.lock().map_err(|_| failure())?;
            action(guard.as_mut().ok_or_else(failure)?)
        })
        .await
        .map_err(|_| failure())?
    }
}

#[tauri::command]
pub(crate) fn history_status(state: tauri::State<'_, History>) -> Option<String> {
    state.status.lock().ok().and_then(|status| status.clone())
}

#[tauri::command]
pub(crate) async fn list_history(
    state: tauri::State<'_, History>,
    before: Option<String>,
) -> Result<Vec<HistoryEntry>, RunError> {
    state
        .call(move |store| {
            let cursor = before
                .map(|id| store.cursor(&id))
                .transpose()?
                .unwrap_or(i64::MAX);
            store.history(cursor)
        })
        .await
}

#[tauri::command]
pub(crate) async fn load_history(
    state: tauri::State<'_, History>,
    id: String,
) -> Result<HistoryEntry, RunError> {
    state.call(move |store| store.detail(&id)).await
}

#[tauri::command]
pub(crate) async fn delete_history(
    state: tauri::State<'_, History>,
    id: String,
) -> Result<(), RunError> {
    state.call(move |store| store.delete_run(&id)).await
}

#[tauri::command]
pub(crate) async fn list_configurations(
    state: tauri::State<'_, History>,
    before: Option<String>,
) -> Result<Vec<SavedConfiguration>, RunError> {
    state
        .call(move |store| store.configurations(before.as_deref()))
        .await
}

#[tauri::command]
pub(crate) async fn save_configuration(
    state: tauri::State<'_, History>,
    name: String,
    config: RunConfig,
    remember: bool,
) -> Result<(), RunError> {
    if name.trim().is_empty() || name.chars().count() > 80 {
        return Err(RunError {
            code: "INVALID_CONFIGURATION_NAME".into(),
            message: "Enter a name between 1 and 80 characters.".into(),
            field: Some("name".into()),
        });
    }
    state
        .call(move |store| {
            let confirmation = velo_engine::RunManager::prepare(config.clone())?;
            let id = uuid::Uuid::new_v4().to_string();
            if remember {
                credentials::remember(&id, &config)?;
            }
            if let Err(error) = store.save(
                &id,
                name.trim(),
                remember,
                &redacted(&config, &confirmation.origin),
            ) {
                if remember {
                    let _ = credentials::forget(&id);
                }
                return Err(error);
            }
            Ok(())
        })
        .await
}

#[tauri::command]
pub(crate) async fn load_configuration(
    state: tauri::State<'_, History>,
    id: String,
) -> Result<SavedConfiguration, RunError> {
    state
        .call(move |store| {
            let mut saved = store.configuration(&id)?;
            if saved.remembered {
                let mut config: RunConfig =
                    serde_json::from_value(saved.config).map_err(|_| failure())?;
                credentials::restore(&id, &mut config)?;
                saved.config = serde_json::to_value(config).map_err(|_| failure())?;
            }
            Ok(saved)
        })
        .await
}

#[tauri::command]
pub(crate) async fn delete_configuration(
    state: tauri::State<'_, History>,
    id: String,
) -> Result<(), RunError> {
    state
        .call(move |store| {
            let saved = store.configuration(&id)?;
            if saved.remembered {
                credentials::forget(&id)?;
            }
            store.delete_configuration(&id)
        })
        .await
}

pub(crate) fn initialize<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let path = app.path().app_data_dir().ok().and_then(|directory| {
        std::fs::create_dir_all(&directory).ok()?;
        Some(directory.join("velo.sqlite3"))
    });
    app.manage(History::new(path));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_store_and_admission_saturation_return_safe_errors() {
        let history = History::new(None);
        assert_eq!(
            tauri::async_runtime::block_on(history.call(|store| store.history(i64::MAX)))
                .unwrap_err()
                .code,
            "STORAGE_UNAVAILABLE"
        );
        let _permits = history.admission.try_acquire_many(4).unwrap();
        assert_eq!(
            tauri::async_runtime::block_on(history.call(|store| store.history(i64::MAX)))
                .unwrap_err()
                .code,
            "STORAGE_UNAVAILABLE"
        );
    }
    fn sample() -> RunUpdate {
        tauri::async_runtime::block_on(async {
            let manager = velo_engine::RunManager::default();
            let snapshot = manager
                .start(
                    RunConfig {
                        url: "http://127.0.0.1:9/".into(),
                        workload: velo_engine::Workload::Constant {
                            vus: 1,
                            duration_ms: 1,
                        },
                        grace_ms: 0,
                        ..RunConfig::default()
                    },
                    true,
                )
                .await
                .unwrap();
            manager.shutdown().await;
            manager.sample(&snapshot.run_id).await.unwrap()
        })
    }

    #[test]
    fn bounded_queue_reports_backpressure_without_waiting_for_storage() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let history = History {
            store: Arc::new(Mutex::new(None)),
            admission: Arc::new(tokio::sync::Semaphore::new(4)),
            sender,
            status: Arc::new(Mutex::new(None)),
        };
        let update = sample();
        assert!(history.record("run", update.clone(), false));
        assert!(!history.record("run", update, false));
        assert_eq!(
            history.status.lock().unwrap().as_deref(),
            Some("HISTORY_BACKPRESSURE")
        );
        assert_eq!(receiver.try_recv().unwrap().0, "run");
    }

    #[test]
    fn finalization_waits_for_queued_writes_and_terminal_commit() {
        let directory = tempfile::tempdir().unwrap();
        let history = History::new(Some(directory.path().join("history.sqlite3")));
        let mut update = sample();
        tauri::async_runtime::block_on(async {
            assert!(
                history
                    .begin("run".into(), "GET".into(), update.snapshot.clone())
                    .await
            );
            update.sequence = "1".into();
            assert!(history.record("run", update.clone(), false));
            update.sequence = "2".into();
            history.finish("run".into(), update, true).await;
            let entry = history.call(|store| store.detail("run")).await.unwrap();
            assert!(entry.incomplete);
            assert_eq!(entry.points.len(), 2);
            assert_eq!(entry.snapshot["state"], "cancelled");
        });
    }
}

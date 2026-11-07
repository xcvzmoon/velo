use std::sync::{Arc, Mutex, mpsc};

use tauri::Manager;
use velo_contracts::{HistoryEntry, SavedConfiguration};
#[cfg(test)]
use velo_engine::RunConfig;
use velo_engine::{RunError, RunSnapshot, RunUpdate};

use crate::{
    credentials::NativeCredentials,
    saved_requests,
    storage::{Store, failure, failure_from},
};

/// Bounds UI waits beyond SQLite's two-second busy timeout; blocking work retains its permit.
const STORAGE_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

pub(crate) type SharedStore = Arc<Mutex<Option<Store>>>;

pub(crate) struct History {
    store: SharedStore,
    admission: Arc<tokio::sync::Semaphore>,
    credentials: Arc<tokio::sync::Semaphore>,
    sender: mpsc::SyncSender<(
        String,
        RunUpdate,
        bool,
        Option<tokio::sync::oneshot::Sender<()>>,
    )>,
    status: Arc<Mutex<Option<String>>>,
}

impl History {
    pub(crate) fn new(path: Option<std::path::PathBuf>) -> Self {
        let opened = path.and_then(|path| {
            Store::open(&path)
                .map_err(|error| {
                    tracing::warn!(code = error.code, "History initialization failed");
                })
                .ok()
        });
        let status = Arc::new(Mutex::new(if opened.is_some() {
            None
        } else {
            Some("STORAGE_UNAVAILABLE".into())
        }));
        let recover_pending = opened.as_ref().is_some_and(|store| {
            store
                .pending_cleanup()
                .map_or(true, |pending| !pending.is_empty())
        });
        let store = Arc::new(Mutex::new(opened));
        let (sender, receiver) = mpsc::sync_channel::<(
            String,
            RunUpdate,
            bool,
            Option<tokio::sync::oneshot::Sender<()>>,
        )>(32);
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
        let credentials = Arc::new(tokio::sync::Semaphore::new(1));
        let recovery_store = store.clone();
        let recovery_credentials = credentials.clone();
        let recovery_status = status.clone();
        if recover_pending {
            std::thread::spawn(move || {
                if let Ok(_permit) = recovery_credentials.try_acquire_owned()
                    && let Err(error) = saved_requests::recover(&recovery_store, &NativeCredentials)
                    && error.code == "CREDENTIAL_CLEANUP_PENDING"
                    && let Ok(mut status) = recovery_status.lock()
                {
                    *status = Some(error.code);
                }
            });
        }
        Self {
            credentials,
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
        self.finish_with_deadline(id, update, incomplete, STORAGE_WAIT)
            .await;
    }

    async fn finish_with_deadline(
        &self,
        id: String,
        update: RunUpdate,
        incomplete: bool,
        deadline: std::time::Duration,
    ) {
        let (acknowledged, received) = tokio::sync::oneshot::channel();
        let committed = if self
            .sender
            .try_send((id, update, incomplete, Some(acknowledged)))
            .is_ok()
        {
            matches!(tokio::time::timeout(deadline, received).await, Ok(Ok(())))
        } else {
            false
        };
        if !committed && let Ok(mut status) = self.status.lock() {
            *status = Some("HISTORY_WRITE_FAILED".into());
        }
    }

    pub(crate) async fn configuration<T: Send + 'static>(
        &self,
        credential_access: bool,
        action: impl FnOnce(&SharedStore) -> Result<T, RunError> + Send + 'static,
    ) -> Result<T, RunError> {
        let admission = if credential_access {
            &self.credentials
        } else {
            &self.admission
        };
        let permit = admission.clone().try_acquire_owned().map_err(|_| {
            if credential_access {
                RunError {
                    code: "CREDENTIALS_BUSY".into(),
                    message:
                        "Another credential operation is still running. Try again when it finishes."
                            .into(),
                    field: None,
                }
            } else {
                failure()
            }
        })?;
        let store = self.store.clone();
        let status = self.status.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            let result = action(&store);
            let cleanup_complete = saved_requests::access(&store, |store| store.pending_cleanup())
                .is_ok_and(|pending| pending.is_empty());
            if let Ok(mut status) = status.lock() {
                if result
                    .as_ref()
                    .is_err_and(|error| error.code == "CREDENTIAL_CLEANUP_PENDING")
                {
                    *status = Some("CREDENTIAL_CLEANUP_PENDING".into());
                } else if cleanup_complete
                    && status.as_deref() == Some("CREDENTIAL_CLEANUP_PENDING")
                {
                    *status = None;
                }
            }
            result
        })
        .await
        .map_err(failure_from)?
    }

    pub(crate) async fn call<T: Send + 'static>(
        &self,
        action: impl FnOnce(&mut Store) -> Result<T, RunError> + Send + 'static,
    ) -> Result<T, RunError> {
        let permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| failure())?;
        let store = self.store.clone();
        let task = tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            saved_requests::access(&store, action)
        });
        tokio::time::timeout(STORAGE_WAIT, task)
            .await
            .map_err(|_| {
                tracing::warn!(
                    code = "STORAGE_WAIT_EXCEEDED",
                    "Storage operation exceeded its response deadline"
                );
                failure()
            })?
            .map_err(failure_from)?
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

#[cfg(test)]
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
        .configuration(remember, move |store| {
            saved_requests::save(store, &NativeCredentials, name.trim(), config, remember)
        })
        .await
}

#[tauri::command]
pub(crate) async fn load_configuration(
    state: tauri::State<'_, History>,
    id: String,
) -> Result<SavedConfiguration, RunError> {
    let lookup = id.clone();
    let saved = state
        .call(move |store| store.configuration(&lookup))
        .await?;
    if !saved.remembered {
        return Ok(saved);
    }
    state
        .configuration(true, move |store| {
            saved_requests::load(store, &NativeCredentials, &id)
        })
        .await
}

#[tauri::command]
pub(crate) async fn delete_configuration(
    state: tauri::State<'_, History>,
    id: String,
) -> Result<(), RunError> {
    let lookup = id.clone();
    let saved = state
        .call(move |store| store.configuration(&lookup))
        .await?;
    state
        .configuration(saved.remembered, move |store| {
            saved_requests::remove(store, &NativeCredentials, &id)
        })
        .await
}

pub(crate) fn initialize<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let path = app
        .path()
        .app_data_dir()
        .map_err(failure_from)
        .ok()
        .and_then(|directory| {
            std::fs::create_dir_all(&directory)
                .map_err(failure_from)
                .ok()?;
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
            credentials: Arc::new(tokio::sync::Semaphore::new(1)),
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
    fn pending_cleanup_does_not_block_omitted_request_operations() {
        let directory = tempfile::tempdir().unwrap();
        let history = History::new(Some(directory.path().join("history.sqlite3")));
        let _credential_operation = history.credentials.try_acquire().unwrap();
        tauri::async_runtime::block_on(async {
            history
                .call(|store| store.queue_cleanup("pending-credential"))
                .await
                .unwrap();
            *history.status.lock().unwrap() = Some("CREDENTIAL_CLEANUP_PENDING".into());
            history
                .configuration(false, |store| {
                    saved_requests::save(
                        store,
                        &NativeCredentials,
                        "Independent",
                        RunConfig {
                            url: "http://127.0.0.1:9/".into(),
                            ..Default::default()
                        },
                        false,
                    )
                })
                .await
                .unwrap();
            let saved = history
                .call(|store| store.configurations(None))
                .await
                .unwrap();
            let id = saved[0].id.clone();
            assert!(!saved[0].remembered);
            history
                .configuration(false, move |store| {
                    saved_requests::remove(store, &NativeCredentials, &id)
                })
                .await
                .unwrap();
            assert_eq!(
                history.status.lock().unwrap().as_deref(),
                Some("CREDENTIAL_CLEANUP_PENDING")
            );
        });
    }

    #[test]
    fn finalization_reports_a_stalled_writer_without_waiting_forever() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let history = History {
            store: Arc::new(Mutex::new(None)),
            admission: Arc::new(tokio::sync::Semaphore::new(4)),
            credentials: Arc::new(tokio::sync::Semaphore::new(1)),
            sender,
            status: Arc::new(Mutex::new(None)),
        };
        tauri::async_runtime::block_on(history.finish_with_deadline(
            "run".into(),
            sample(),
            false,
            std::time::Duration::from_millis(5),
        ));
        assert_eq!(
            history.status.lock().unwrap().as_deref(),
            Some("HISTORY_WRITE_FAILED")
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

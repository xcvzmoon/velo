#[cfg(test)]
use crate::storage::redacted;
use velo_contracts::SavedConfiguration;
use velo_engine::{RunConfig, RunError};

use crate::{
    credentials::Credentials,
    history::SharedStore,
    storage::{Store, failure, failure_from},
};

pub(crate) fn access<T>(
    store: &SharedStore,
    action: impl FnOnce(&mut Store) -> Result<T, RunError>,
) -> Result<T, RunError> {
    let mut guard = store.lock().map_err(|_| failure())?;
    action(guard.as_mut().ok_or_else(failure)?)
}

fn cleanup_pending() -> RunError {
    tracing::warn!(
        code = "CREDENTIAL_CLEANUP_PENDING",
        "Credential cleanup will be retried"
    );
    RunError {
        code: "CREDENTIAL_CLEANUP_PENDING".into(),
        message: "The request was removed or could not be saved, but OS credential cleanup remains pending. Restart Velo to retry cleanup.".into(),
        field: None,
    }
}

fn cleanup(store: &SharedStore, credentials: &impl Credentials, id: &str) -> Result<(), RunError> {
    credentials.forget(id).map_err(|_| cleanup_pending())?;
    access(store, |store| store.complete_cleanup(id)).map_err(|_| cleanup_pending())
}

pub(crate) fn recover(store: &SharedStore, credentials: &impl Credentials) -> Result<(), RunError> {
    loop {
        let pending = access(store, |store| store.pending_cleanup())?;
        if pending.is_empty() {
            return Ok(());
        }
        for id in pending {
            cleanup(store, credentials, &id)?;
        }
    }
}

#[cfg(test)]
pub(crate) fn save(
    store: &SharedStore,
    credentials: &impl Credentials,
    name: &str,
    config: RunConfig,
    remember: bool,
) -> Result<(), RunError> {
    let confirmation = velo_engine::RunManager::prepare(config.clone())?;
    let id = uuid::Uuid::new_v4().to_string();
    if remember {
        access(store, |store| store.queue_cleanup(&id))?;
        if let Err(error) = credentials.remember(&id, &config) {
            cleanup(store, credentials, &id)?;
            return Err(error);
        }
    }
    if let Err(error) = access(store, |store| {
        store.save(
            &id,
            name,
            remember,
            &redacted(&config, &confirmation.origin),
        )
    }) {
        if remember {
            cleanup(store, credentials, &id)?;
        }
        return Err(error);
    }
    Ok(())
}

pub(crate) fn load(
    store: &SharedStore,
    credentials: &impl Credentials,
    id: &str,
) -> Result<SavedConfiguration, RunError> {
    let mut saved = access(store, |store| store.configuration(id))?;
    if saved.remembered {
        let mut config: RunConfig = serde_json::from_value(saved.config).map_err(failure_from)?;
        credentials.restore(id, &mut config)?;
        saved.config = serde_json::to_value(config).map_err(failure_from)?;
    }
    Ok(saved)
}

pub(crate) fn remove(
    store: &SharedStore,
    credentials: &impl Credentials,
    id: &str,
) -> Result<(), RunError> {
    let saved = access(store, |store| store.configuration(id))?;
    access(store, |store| store.delete_configuration(id))?;
    if saved.remembered {
        cleanup(store, credentials, id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::Path,
        sync::{Arc, Mutex, mpsc},
        time::Duration,
    };

    use rusqlite::Connection;

    use super::*;

    #[derive(Default)]
    struct MemoryState {
        entries: HashMap<String, RunConfig>,
        fail_remember: bool,
        fail_forget: bool,
    }

    struct RememberGate {
        started: mpsc::SyncSender<()>,
        release: Mutex<mpsc::Receiver<()>>,
    }

    struct MemoryCredentials {
        store: SharedStore,
        state: Mutex<MemoryState>,
        gate: Option<RememberGate>,
    }

    impl MemoryCredentials {
        fn new(store: &SharedStore) -> Self {
            Self {
                store: store.clone(),
                state: Mutex::default(),
                gate: None,
            }
        }
    }

    fn credential_failure() -> RunError {
        RunError {
            code: "CREDENTIALS_UNAVAILABLE".into(),
            message: "Injected credential failure".into(),
            field: None,
        }
    }

    impl Credentials for MemoryCredentials {
        fn remember(&self, id: &str, config: &RunConfig) -> Result<(), RunError> {
            assert!(
                self.store.try_lock().is_ok(),
                "database lock held during remember"
            );
            if let Some(gate) = &self.gate {
                gate.started.send(()).unwrap();
                gate.release
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
            }
            let mut state = self.state.lock().unwrap();
            state.entries.insert(id.into(), config.clone());
            if state.fail_remember {
                return Err(credential_failure());
            }
            Ok(())
        }

        fn restore(&self, id: &str, config: &mut RunConfig) -> Result<(), RunError> {
            assert!(
                self.store.try_lock().is_ok(),
                "database lock held during restore"
            );
            *config = self
                .state
                .lock()
                .unwrap()
                .entries
                .get(id)
                .cloned()
                .ok_or_else(credential_failure)?;
            Ok(())
        }

        fn forget(&self, id: &str) -> Result<(), RunError> {
            assert!(
                self.store.try_lock().is_ok(),
                "database lock held during forget"
            );
            let mut state = self.state.lock().unwrap();
            if state.fail_forget {
                return Err(credential_failure());
            }
            state.entries.remove(id);
            Ok(())
        }
    }

    fn open(path: &Path) -> SharedStore {
        Arc::new(Mutex::new(Some(Store::open(path).unwrap())))
    }

    fn reopen(store: &SharedStore, path: &Path) {
        let mut guard = store.lock().unwrap();
        drop(guard.take());
        *guard = Some(Store::open(path).unwrap());
    }

    fn config() -> RunConfig {
        RunConfig {
            url: "https://example.com/private?token=secret".into(),
            ..RunConfig::default()
        }
    }

    fn configuration_id(store: &SharedStore) -> String {
        access(store, |store| Ok(store.configurations(None)?.remove(0).id)).unwrap()
    }

    #[test]
    fn blocked_credential_io_leaves_database_available_and_round_trip_restores_request() {
        let directory = tempfile::tempdir().unwrap();
        let store = open(&directory.path().join("history.sqlite3"));
        let (started, observed) = mpsc::sync_channel(1);
        let (release, waiting) = mpsc::sync_channel(1);
        let mut credentials = MemoryCredentials::new(&store);
        credentials.gate = Some(RememberGate {
            started,
            release: Mutex::new(waiting),
        });
        let credentials = Arc::new(credentials);
        let operation_store = store.clone();
        let operation_credentials = credentials.clone();
        let operation = std::thread::spawn(move || {
            save(
                &operation_store,
                operation_credentials.as_ref(),
                "Saved",
                config(),
                true,
            )
        });
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        let database_available = store.try_lock().is_ok();
        release.send(()).unwrap();
        operation.join().unwrap().unwrap();
        assert!(database_available);
        let id = configuration_id(&store);
        let saved = load(&store, credentials.as_ref(), &id).unwrap();
        assert_eq!(saved.config["url"], config().url);
        remove(&store, credentials.as_ref(), &id).unwrap();
        assert!(credentials.state.lock().unwrap().entries.is_empty());
        assert!(
            access(&store, |store| store.pending_cleanup())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn failed_save_and_failed_cleanup_remain_recoverable_after_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let store = open(&path);
        let credentials = MemoryCredentials::new(&store);
        credentials.state.lock().unwrap().fail_forget = true;
        let injector = Connection::open(&path).unwrap();
        injector.execute_batch("CREATE TRIGGER reject_save BEFORE INSERT ON configurations BEGIN SELECT RAISE(ABORT, 'injected save failure'); END;").unwrap();
        let error = save(&store, &credentials, "Saved", config(), true).unwrap_err();
        assert_eq!(error.code, "CREDENTIAL_CLEANUP_PENDING");
        let pending = access(&store, |store| store.pending_cleanup()).unwrap();
        assert_eq!(pending.len(), 1);
        assert!(
            credentials
                .state
                .lock()
                .unwrap()
                .entries
                .contains_key(&pending[0])
        );
        assert!(
            access(&store, |store| store.configurations(None))
                .unwrap()
                .is_empty()
        );
        drop(injector);
        reopen(&store, &path);
        assert_eq!(
            access(&store, |store| store.pending_cleanup()).unwrap(),
            pending
        );
        credentials.state.lock().unwrap().fail_forget = false;
        recover(&store, &credentials).unwrap();
        assert!(
            access(&store, |store| store.pending_cleanup())
                .unwrap()
                .is_empty()
        );
        assert!(credentials.state.lock().unwrap().entries.is_empty());
    }

    #[test]
    fn failed_database_delete_preserves_configuration_and_credentials() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let store = open(&path);
        let credentials = MemoryCredentials::new(&store);
        save(&store, &credentials, "Saved", config(), true).unwrap();
        let id = configuration_id(&store);
        let injector = Connection::open(&path).unwrap();
        injector.execute_batch("CREATE TRIGGER reject_delete BEFORE DELETE ON configurations BEGIN SELECT RAISE(ABORT, 'injected delete failure'); END;").unwrap();
        assert_eq!(
            remove(&store, &credentials, &id).unwrap_err().code,
            "STORAGE_UNAVAILABLE"
        );
        assert!(credentials.state.lock().unwrap().entries.contains_key(&id));
        assert_eq!(
            load(&store, &credentials, &id).unwrap().config["url"],
            config().url
        );
        assert!(
            access(&store, |store| store.pending_cleanup())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn failed_keychain_delete_is_journaled_and_retried_after_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let store = open(&path);
        let credentials = MemoryCredentials::new(&store);
        save(&store, &credentials, "Saved", config(), true).unwrap();
        let id = configuration_id(&store);
        credentials.state.lock().unwrap().fail_forget = true;
        assert_eq!(
            remove(&store, &credentials, &id).unwrap_err().code,
            "CREDENTIAL_CLEANUP_PENDING"
        );
        assert!(
            access(&store, |store| store.configurations(None))
                .unwrap()
                .is_empty()
        );
        assert!(credentials.state.lock().unwrap().entries.contains_key(&id));
        reopen(&store, &path);
        assert_eq!(
            access(&store, |store| store.pending_cleanup()).unwrap(),
            vec![id]
        );
        credentials.state.lock().unwrap().fail_forget = false;
        recover(&store, &credentials).unwrap();
        assert!(credentials.state.lock().unwrap().entries.is_empty());
        assert!(
            access(&store, |store| store.pending_cleanup())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn ambiguous_credential_write_failure_cleans_up_before_returning_original_error() {
        let directory = tempfile::tempdir().unwrap();
        let store = open(&directory.path().join("history.sqlite3"));
        let credentials = MemoryCredentials::new(&store);
        credentials.state.lock().unwrap().fail_remember = true;
        assert_eq!(
            save(&store, &credentials, "Saved", config(), true)
                .unwrap_err()
                .code,
            "CREDENTIALS_UNAVAILABLE"
        );
        assert!(credentials.state.lock().unwrap().entries.is_empty());
        assert!(
            access(&store, |store| store.configurations(None))
                .unwrap()
                .is_empty()
        );
        assert!(
            access(&store, |store| store.pending_cleanup())
                .unwrap()
                .is_empty()
        );
    }
}

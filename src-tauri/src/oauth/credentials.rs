//! Cancellation-aware credential commits with restoration of the previous local record.

use tokio_util::sync::CancellationToken;
use velo_engine::RunError;

use super::failure;

pub(super) trait TokenEntry {
    fn read(&self) -> keyring::Result<String>;
    fn write(&self, value: &str) -> keyring::Result<()>;
    fn delete(&self) -> keyring::Result<()>;
}

impl TokenEntry for keyring::Entry {
    fn read(&self) -> keyring::Result<String> {
        self.get_password()
    }
    fn write(&self, value: &str) -> keyring::Result<()> {
        self.set_password(value)
    }
    fn delete(&self) -> keyring::Result<()> {
        self.delete_credential()
    }
}

/// Commits a local record while retaining the previous value for cancellation or write failure.
/// The caller must await the blocking worker, including rollback; dropping its async owner cannot
/// cancel an OS credential operation. `OAUTH_CREDENTIAL_ROLLBACK_FAILED` requires explicit cleanup.
pub(super) fn commit(
    entry: &impl TokenEntry,
    value: &str,
    cancel: &CancellationToken,
) -> Result<(), RunError> {
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    let previous = match entry.read() {
        Ok(value) => Some(value),
        Err(keyring::Error::NoEntry) => None,
        Err(_) => return Err(unavailable()),
    };
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    let written = entry.write(value);
    if written.is_ok() && !cancel.is_cancelled() {
        return Ok(());
    }
    let restored = match previous {
        Some(previous) => entry.write(&previous),
        None => match entry.delete() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error),
        },
    };
    if restored.is_err() {
        return Err(failure(
            "OAUTH_CREDENTIAL_ROLLBACK_FAILED",
            "The OS credential operation could not be rolled back. Forget this local token before acquiring it again.",
        ));
    }
    if cancel.is_cancelled() {
        Err(cancelled())
    } else {
        Err(unavailable())
    }
}

fn cancelled() -> RunError {
    failure("OAUTH_CANCELLED", "Token acquisition was cancelled.")
}
fn unavailable() -> RunError {
    failure(
        "OAUTH_CREDENTIAL_UNAVAILABLE",
        "The token could not be saved in the OS credential store.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, mpsc};

    struct PausedEntry {
        value: Mutex<Option<String>>,
        started: mpsc::Sender<()>,
        resume: Mutex<mpsc::Receiver<()>>,
        fail_rollback: bool,
    }

    impl TokenEntry for PausedEntry {
        fn read(&self) -> keyring::Result<String> {
            self.value
                .lock()
                .unwrap()
                .clone()
                .ok_or(keyring::Error::NoEntry)
        }
        fn write(&self, value: &str) -> keyring::Result<()> {
            if value == "new-token" {
                *self.value.lock().unwrap() = Some(value.into());
                self.started.send(()).unwrap();
                self.resume.lock().unwrap().recv().unwrap();
                Ok(())
            } else if self.fail_rollback {
                Err(keyring::Error::Invalid("store".into(), "denied".into()))
            } else {
                *self.value.lock().unwrap() = Some(value.into());
                Ok(())
            }
        }
        fn delete(&self) -> keyring::Result<()> {
            if self.fail_rollback {
                return Err(keyring::Error::Invalid("store".into(), "denied".into()));
            }
            self.value.lock().unwrap().take();
            Ok(())
        }
    }

    #[test]
    fn cancellation_during_os_write_waits_and_restores_or_removes_the_record() {
        for previous in [None, Some("old-token".to_owned())] {
            let (started, receive_started) = mpsc::channel();
            let (resume, receive_resume) = mpsc::channel();
            let entry = PausedEntry {
                value: Mutex::new(previous.clone()),
                started,
                resume: Mutex::new(receive_resume),
                fail_rollback: false,
            };
            let cancel = CancellationToken::new();
            std::thread::scope(|scope| {
                let worker = scope.spawn(|| commit(&entry, "new-token", &cancel));
                receive_started.recv().unwrap();
                cancel.cancel();
                assert!(!worker.is_finished());
                resume.send(()).unwrap();
                assert_eq!(worker.join().unwrap().unwrap_err().code, "OAUTH_CANCELLED");
            });
            assert_eq!(*entry.value.lock().unwrap(), previous);
        }
    }

    #[test]
    fn denied_rollback_is_reported_without_claiming_cleanup_succeeded() {
        let (started, receive_started) = mpsc::channel();
        let (resume, receive_resume) = mpsc::channel();
        let entry = PausedEntry {
            value: Mutex::new(None),
            started,
            resume: Mutex::new(receive_resume),
            fail_rollback: true,
        };
        let cancel = CancellationToken::new();
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| commit(&entry, "new-token", &cancel));
            receive_started.recv().unwrap();
            cancel.cancel();
            resume.send(()).unwrap();
            let error = worker.join().unwrap().unwrap_err();
            assert_eq!(error.code, "OAUTH_CREDENTIAL_ROLLBACK_FAILED");
            assert!(!error.message.contains("new-token"));
        });
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::super::{Flow, OAuthState, oauth_cancel};
    use super::*;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;
    use tauri::Manager;

    struct BlockingEntry {
        value: Mutex<Option<String>>,
        started: mpsc::Sender<()>,
        resume: Mutex<mpsc::Receiver<()>>,
    }

    impl TokenEntry for BlockingEntry {
        fn read(&self) -> keyring::Result<String> {
            self.value
                .lock()
                .unwrap()
                .clone()
                .ok_or(keyring::Error::NoEntry)
        }
        fn write(&self, value: &str) -> keyring::Result<()> {
            *self.value.lock().unwrap() = Some(value.into());
            self.started.send(()).unwrap();
            self.resume.lock().unwrap().recv().unwrap();
            Ok(())
        }
        fn delete(&self) -> keyring::Result<()> {
            self.value.lock().unwrap().take();
            Ok(())
        }
    }

    #[tokio::test]
    async fn native_cancellation_awaits_the_os_worker_and_its_rollback() {
        let app = tauri::test::mock_builder()
            .manage(OAuthState::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let (started, receive_started) = mpsc::channel();
        let (resume, receive_resume) = mpsc::channel();
        let entry = Arc::new(BlockingEntry {
            value: Mutex::new(None),
            started,
            resume: Mutex::new(receive_resume),
        });
        let cancellation = CancellationToken::new();
        let owned_cancel = cancellation.clone();
        let owned_entry = entry.clone();
        let task = tauri::async_runtime::spawn(async move {
            let result = tauri::async_runtime::spawn_blocking(move || {
                commit(owned_entry.as_ref(), "new-token", &owned_cancel)
            })
            .await
            .unwrap();
            assert_eq!(result.unwrap_err().code, "OAUTH_CANCELLED");
            Ok(())
        });
        *app.state::<OAuthState>().0.lock().await = Some(Flow {
            id: "flow".into(),
            cancel: cancellation.clone(),
            result: None,
            task: Some(task),
        });
        receive_started
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        let cancel = oauth_cancel(app.state::<OAuthState>(), "flow".into());
        tokio::pin!(cancel);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut cancel)
                .await
                .is_err()
        );
        assert!(cancellation.is_cancelled());
        resume.send(()).unwrap();
        cancel.await.unwrap();
        assert!(entry.value.lock().unwrap().is_none());
        assert!(app.state::<OAuthState>().0.lock().await.is_none());
    }
}

#[cfg(test)]
mod failure_tests {
    use super::super::{Flow, OAuthState, oauth_cancel};
    use super::*;
    use std::sync::Mutex;
    use tauri::Manager;

    struct AmbiguousEntry(Mutex<Option<String>>);
    impl TokenEntry for AmbiguousEntry {
        fn read(&self) -> keyring::Result<String> {
            self.0
                .lock()
                .unwrap()
                .clone()
                .ok_or(keyring::Error::NoEntry)
        }
        fn write(&self, value: &str) -> keyring::Result<()> {
            *self.0.lock().unwrap() = Some(value.into());
            if value == "new-token" {
                Err(keyring::Error::Invalid(
                    "store".into(),
                    "ambiguous failure".into(),
                ))
            } else {
                Ok(())
            }
        }
        fn delete(&self) -> keyring::Result<()> {
            self.0.lock().unwrap().take();
            Ok(())
        }
    }

    #[test]
    fn ambiguous_write_failures_restore_previous_credentials() {
        for previous in [None, Some("old-token".to_owned())] {
            let entry = AmbiguousEntry(Mutex::new(previous.clone()));
            let error = commit(&entry, "new-token", &CancellationToken::new()).unwrap_err();
            assert_eq!(error.code, "OAUTH_CREDENTIAL_UNAVAILABLE");
            assert_eq!(*entry.0.lock().unwrap(), previous);
        }
    }

    #[tokio::test]
    async fn native_cancellation_reports_rollback_failure_instead_of_success() {
        let app = tauri::test::mock_builder()
            .manage(OAuthState::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let task = tauri::async_runtime::spawn(async {
            Err(failure(
                "OAUTH_CREDENTIAL_ROLLBACK_FAILED",
                "Cleanup failed.",
            ))
        });
        *app.state::<OAuthState>().0.lock().await = Some(Flow {
            id: "flow".into(),
            cancel: CancellationToken::new(),
            result: None,
            task: Some(task),
        });
        let error = oauth_cancel(app.state::<OAuthState>(), "flow".into())
            .await
            .unwrap_err();
        assert_eq!(error.code, "OAUTH_CREDENTIAL_ROLLBACK_FAILED");
    }
}

use keyring::Entry;
use velo_engine::{RunConfig, RunError};

const SERVICE: &str = "app.velo.saved-configurations";
const MAX_SECRET_BYTES: usize = 65_536;

trait CredentialEntry {
    fn write(&self, value: &str) -> keyring::Result<()>;
    fn read(&self) -> keyring::Result<String>;
    fn delete(&self) -> keyring::Result<()>;
}

impl CredentialEntry for Entry {
    fn write(&self, value: &str) -> keyring::Result<()> {
        self.set_password(value)
    }

    fn read(&self) -> keyring::Result<String> {
        self.get_password()
    }

    fn delete(&self) -> keyring::Result<()> {
        self.delete_credential()
    }
}

pub(crate) fn remember(id: &str, config: &RunConfig) -> Result<(), RunError> {
    remember_with(&entry(id)?, config)
}

pub(crate) fn restore(id: &str, config: &mut RunConfig) -> Result<(), RunError> {
    restore_with(&entry(id)?, config)
}

pub(crate) fn forget(id: &str) -> Result<(), RunError> {
    forget_with(&entry(id)?)
}

fn entry(id: &str) -> Result<Entry, RunError> {
    validate_id(id)?;
    Entry::new(SERVICE, id).map_err(|_| unavailable())
}

fn validate_id(id: &str) -> Result<(), RunError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(RunError {
            code: "INVALID_CONFIGURATION_ID".into(),
            message: "Choose a valid saved configuration.".into(),
            field: Some("id".into()),
        });
    }
    Ok(())
}

fn remember_with(entry: &impl CredentialEntry, config: &RunConfig) -> Result<(), RunError> {
    let secret = serde_json::to_string(config).map_err(|_| invalid())?;
    if secret.len() > MAX_SECRET_BYTES {
        return Err(RunError {
            code: "CREDENTIALS_TOO_LARGE".into(),
            message: "This request is too large to remember securely. Save without credentials and re-enter its request data before running.".into(),
            field: None,
        });
    }
    entry.write(&secret).map_err(|_| unavailable())
}

fn restore_with(entry: &impl CredentialEntry, config: &mut RunConfig) -> Result<(), RunError> {
    let secret = entry.read().map_err(|_| unavailable())?;
    if secret.len() > MAX_SECRET_BYTES {
        return Err(invalid());
    }
    let restored = serde_json::from_str(&secret).map_err(|_| invalid())?;
    *config = restored;
    Ok(())
}

fn forget_with(entry: &impl CredentialEntry) -> Result<(), RunError> {
    match entry.delete() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(unavailable()),
    }
}

fn unavailable() -> RunError {
    RunError {
        code: "CREDENTIALS_UNAVAILABLE".into(),
        message: "The OS credential store is unavailable or the saved entry is missing. Re-enter the request data; it will not be saved as plaintext.".into(),
        field: None,
    }
}

fn invalid() -> RunError {
    RunError {
        code: "CREDENTIALS_INVALID".into(),
        message: "The remembered request could not be restored. Re-enter and save the request data again.".into(),
        field: None,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use velo_engine::{NameValue, RequestAuth, RequestBody, RequestMethod};

    #[derive(Default)]
    struct MemoryEntry {
        secret: RefCell<Option<String>>,
        unavailable: bool,
    }

    impl CredentialEntry for MemoryEntry {
        fn write(&self, value: &str) -> keyring::Result<()> {
            if self.unavailable {
                return Err(keyring::Error::NoEntry);
            }
            *self.secret.borrow_mut() = Some(value.into());
            Ok(())
        }

        fn read(&self) -> keyring::Result<String> {
            if self.unavailable {
                return Err(keyring::Error::NoEntry);
            }
            self.secret.borrow().clone().ok_or(keyring::Error::NoEntry)
        }

        fn delete(&self) -> keyring::Result<()> {
            if self.unavailable {
                return Err(keyring::Error::Invalid(
                    "store".into(),
                    "unavailable".into(),
                ));
            }
            self.secret
                .borrow_mut()
                .take()
                .ok_or(keyring::Error::NoEntry)?;
            Ok(())
        }
    }

    #[test]
    fn remembers_all_sensitive_request_fields_and_deletes_idempotently() {
        let entry = MemoryEntry::default();
        let original = RunConfig {
            url: "https://example.com/private?secret=url-secret".into(),
            method: RequestMethod::POST,
            headers: vec![NameValue {
                name: "X-Private".into(),
                value: "header-secret".into(),
            }],
            query: vec![NameValue {
                name: "private".into(),
                value: "query-secret".into(),
            }],
            body: RequestBody::Text("body-secret".into()),
            auth: RequestAuth::Bearer {
                token: "auth-secret".into(),
            },
            ..RunConfig::default()
        };
        remember_with(&entry, &original).unwrap();
        let mut restored = RunConfig::default();
        restore_with(&entry, &mut restored).unwrap();
        assert_eq!(
            serde_json::to_value(restored).unwrap(),
            serde_json::to_value(original).unwrap()
        );
        forget_with(&entry).unwrap();
        forget_with(&entry).unwrap();
        assert!(entry.secret.borrow().is_none());
    }

    #[test]
    fn unavailable_credentials_leave_configuration_unchanged() {
        let entry = MemoryEntry {
            unavailable: true,
            ..MemoryEntry::default()
        };
        let mut config = RunConfig::default();
        let before = serde_json::to_value(&config).unwrap();
        for error in [
            remember_with(&entry, &config).unwrap_err(),
            restore_with(&entry, &mut config).unwrap_err(),
            forget_with(&entry).unwrap_err(),
        ] {
            assert_eq!(error.code, "CREDENTIALS_UNAVAILABLE");
            assert!(!error.message.contains("auth-secret"));
        }
        assert_eq!(serde_json::to_value(config).unwrap(), before);
    }

    #[test]
    fn corrupt_and_oversized_entries_cannot_replace_the_configuration() {
        let entry = MemoryEntry {
            secret: RefCell::new(Some("body-secret".into())),
            unavailable: false,
        };
        let mut config = RunConfig::default();
        let before = serde_json::to_value(&config).unwrap();
        let error = restore_with(&entry, &mut config).unwrap_err();
        assert_eq!(error.code, "CREDENTIALS_INVALID");
        assert!(!error.message.contains("body-secret"));
        assert_eq!(serde_json::to_value(&config).unwrap(), before);
        config.body = RequestBody::Text("x".repeat(MAX_SECRET_BYTES));
        assert_eq!(
            remember_with(&entry, &config).unwrap_err().code,
            "CREDENTIALS_TOO_LARGE"
        );
        *entry.secret.borrow_mut() = Some("x".repeat(MAX_SECRET_BYTES + 1));
        assert_eq!(
            restore_with(&entry, &mut config).unwrap_err().code,
            "CREDENTIALS_INVALID"
        );
    }

    #[test]
    fn credential_ids_are_bounded_and_cannot_target_other_namespaces() {
        for id in ["", "../other", "with space", "é", &"a".repeat(65)] {
            assert_eq!(
                validate_id(id).unwrap_err().code,
                "INVALID_CONFIGURATION_ID"
            );
        }
        validate_id("config-123-ABC").unwrap();
        validate_id(&"a".repeat(64)).unwrap();
    }
}

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tauri::Manager;
use velo_engine::{
    ImportFormat, ImportedRequest, PreviewResponse, RunConfig, RunError, RunManager, RunState,
    TestDefinition,
};

use crate::{execution::DesktopRuns, history::History};

fn failure(code: &str, message: &str, field: Option<&str>) -> RunError {
    RunError {
        code: code.into(),
        message: message.into(),
        field: field.map(str::to_owned),
    }
}

struct Preparation<'a> {
    state: &'a DesktopRuns,
    cancelled: Arc<AtomicBool>,
}

impl<'a> Preparation<'a> {
    fn new(state: &'a DesktopRuns) -> Result<Self, RunError> {
        if state.closing.load(Ordering::Acquire) {
            return Err(failure(
                "APP_EXITING",
                "The application is shutting down.",
                None,
            ));
        }
        let mut current = state.preparation.lock().map_err(|_| {
            failure(
                "AUTHORING_FAILED",
                "Preparation state is unavailable.",
                None,
            )
        })?;
        if current.is_some() {
            return Err(failure(
                "PREPARATION_BUSY",
                "Wait for the current preparation to finish.",
                None,
            ));
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        *current = Some(cancelled.clone());
        Ok(Self { state, cancelled })
    }
}

impl Drop for Preparation<'_> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Ok(mut current) = self.state.preparation.lock() {
            *current = None;
        }
    }
}

#[tauri::command]
pub(crate) fn cancel_data_preparation(
    state: tauri::State<'_, Arc<DesktopRuns>>,
) -> Result<(), RunError> {
    let current = state.preparation.lock().map_err(|_| {
        failure(
            "AUTHORING_FAILED",
            "Preparation state is unavailable.",
            None,
        )
    })?;
    if let Some(cancelled) = current.as_ref() {
        cancelled.store(true, Ordering::Release);
    }
    Ok(())
}

fn secret_entry(name: &str) -> Result<keyring::Entry, RunError> {
    if name.starts_with("saved.")
        || name.is_empty()
        || name.len() > 80
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(failure(
            "INVALID_SECRET_NAME",
            "Use 1 to 80 letters, digits, dots, underscores or hyphens.",
            Some("secret"),
        ));
    }
    keyring::Entry::new("app.velo.authoring", name).map_err(|_| {
        failure(
            "SECRET_UNAVAILABLE",
            "The OS credential store is unavailable.",
            Some("secret"),
        )
    })
}

fn read_secret(
    name: &str,
    saved: &mut std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<String, RunError> {
    use crate::credentials::Credentials;
    if let Some(reference) = name.strip_prefix("saved.") {
        let (id, pointer) = reference.split_once('.').ok_or_else(|| {
            failure(
                "SECRET_UNAVAILABLE",
                "Load the original saved configuration again.",
                None,
            )
        })?;
        if !saved.contains_key(id) {
            let mut config = RunConfig::default();
            crate::credentials::NativeCredentials.restore(id, &mut config)?;
            let value = serde_json::to_value(config).map_err(|_| {
                failure(
                    "SECRET_UNAVAILABLE",
                    "The saved request could not be restored.",
                    None,
                )
            })?;
            saved.insert(id.to_owned(), value);
        }
        let value = saved.get(id).ok_or_else(|| {
            failure(
                "SECRET_UNAVAILABLE",
                "The saved request is unavailable.",
                None,
            )
        })?;
        return value
            .pointer(&pointer.replace('-', "/"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                failure(
                    "SECRET_UNAVAILABLE",
                    "The saved request field is unavailable.",
                    None,
                )
            });
    }
    secret_entry(name)?.get_password().map_err(|_| {
        failure(
            "SECRET_UNAVAILABLE",
            "Set the referenced secret in the OS credential store.",
            Some("secret"),
        )
    })
}

fn resolve(
    definition: TestDefinition,
    environment: Option<String>,
    rows: Vec<std::collections::BTreeMap<String, String>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(RunConfig, Vec<String>), RunError> {
    let mut cache = std::collections::BTreeMap::new();
    let mut saved = std::collections::BTreeMap::new();
    let config = definition.resolve_data(
        environment.as_deref(),
        &rows,
        || cancelled.load(Ordering::Acquire),
        |name| {
            if let Some(value) = cache.get(name) {
                return Ok(String::clone(value));
            }
            let value = read_secret(name, &mut saved)?;
            cache.insert(name.to_owned(), value.clone());
            Ok(value)
        },
    )?;
    Ok((config, cache.into_values().collect()))
}

async fn resolve_native(
    history: &History,
    definition: TestDefinition,
    environment: Option<String>,
    rows: Vec<std::collections::BTreeMap<String, String>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(RunConfig, Vec<String>), RunError> {
    let credential_access = serde_json::to_string(&definition)
        .map_err(|_| {
            failure(
                "AUTHORING_FAILED",
                "The definition could not be encoded.",
                None,
            )
        })?
        .contains("{{secret:");
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        history.configuration(credential_access, move |_| {
            resolve(definition, environment, rows, cancelled)
        }),
    )
    .await
    .map_err(|_| {
        failure(
            "CREDENTIAL_WAIT_EXCEEDED",
            "The OS credential operation is still running. Wait before retrying.",
            None,
        )
    })?
}

fn reference_values(value: &mut serde_json::Value, id: &str, pointer: &str) {
    match value {
        serde_json::Value::String(text) if !text.is_empty() => {
            *text = format!("{{{{secret:saved.{id}.{pointer}}}}}")
        }
        serde_json::Value::Array(items) => {
            for (index, value) in items.iter_mut().enumerate() {
                reference_values(value, id, &format!("{pointer}-{index}"));
            }
        }
        serde_json::Value::Object(items) => {
            for (key, value) in items {
                if key == "path" {
                    *value = serde_json::Value::String(String::new());
                } else if key != "kind" {
                    reference_values(value, id, &format!("{pointer}-{key}"));
                }
            }
        }
        _ => {}
    }
}

#[tauri::command]
pub(crate) async fn load_configuration_definition<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<TestDefinition, RunError> {
    let saved = crate::history::load_configuration(app.state::<History>(), id.clone()).await?;
    let mut config = saved.config;
    if saved.remembered {
        for field in ["url", "headers", "query", "auth", "body"] {
            reference_values(&mut config[field], &id, &format!("-{field}"));
        }
    }
    let config = serde_json::from_value(config).map_err(|_| {
        failure(
            "AUTHORING_FAILED",
            "The saved request could not be loaded.",
            None,
        )
    })?;
    Ok(TestDefinition {
        config,
        ..Default::default()
    })
}

#[tauri::command]
pub(crate) async fn parse_definition(source: String) -> Result<TestDefinition, RunError> {
    tauri::async_runtime::spawn_blocking(move || TestDefinition::parse(&source))
        .await
        .map_err(|_| failure("AUTHORING_FAILED", "Definition parsing failed.", None))?
}

#[tauri::command]
pub(crate) async fn resolve_definition(
    runs: tauri::State<'_, Arc<DesktopRuns>>,
    state: tauri::State<'_, History>,
    definition: TestDefinition,
    environment: Option<String>,
    rows: Option<Vec<std::collections::BTreeMap<String, String>>>,
) -> Result<RunConfig, RunError> {
    let preparation = Preparation::new(&runs)?;
    let cancelled = preparation.cancelled.clone();
    let (config, _) = resolve_native(
        &state,
        definition,
        environment,
        rows.unwrap_or_default(),
        cancelled.clone(),
    )
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        RunManager::prepare(config.clone())?;
        if cancelled.load(Ordering::Acquire) {
            return Err(failure(
                "PREPARATION_CANCELLED",
                "Prepared data was discarded.",
                Some("dataset"),
            ));
        }
        Ok(config)
    })
    .await
    .map_err(|_| failure("AUTHORING_FAILED", "Request preparation failed.", None))?
}

#[tauri::command]
pub(crate) async fn import_requests(
    format: ImportFormat,
    source: String,
) -> Result<Vec<ImportedRequest>, RunError> {
    tauri::async_runtime::spawn_blocking(move || velo_engine::import_requests(format, &source))
        .await
        .map_err(|_| failure("AUTHORING_FAILED", "Request import failed.", None))?
}

#[tauri::command]
pub(crate) async fn set_authoring_secret(
    state: tauri::State<'_, History>,
    name: String,
    value: Option<String>,
) -> Result<(), RunError> {
    let operation = state.configuration(true, move |_| {
        let entry = secret_entry(&name)?;
        match value {
            Some(value) if value.is_empty() || value.len() > 16_384 => Err(failure(
                "INVALID_SECRET_VALUE",
                "Enter between 1 and 16,384 bytes.",
                Some("secret"),
            )),
            Some(value) => entry.set_password(&value).map_err(|_| {
                failure(
                    "SECRET_UNAVAILABLE",
                    "The secret could not be stored securely.",
                    Some("secret"),
                )
            }),
            None => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err(failure(
                    "SECRET_UNAVAILABLE",
                    "The secret could not be deleted.",
                    Some("secret"),
                )),
            },
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), operation)
        .await
        .map_err(|_| {
            failure(
                "CREDENTIAL_WAIT_EXCEEDED",
                "The OS credential operation is still running. Wait before retrying.",
                None,
            )
        })?
}

#[tauri::command]
pub(crate) async fn preview_request(
    history: tauri::State<'_, History>,
    state: tauri::State<'_, Arc<DesktopRuns>>,
    definition: TestDefinition,
    environment: Option<String>,
    authorized: bool,
    rows: Option<Vec<std::collections::BTreeMap<String, String>>>,
) -> Result<PreviewResponse, RunError> {
    let _admission = state.admission.try_lock().map_err(|_| {
        failure(
            "REQUEST_BUSY",
            "Another request operation is active. Wait for it to finish.",
            None,
        )
    })?;
    let exiting = state.preview_exit.notified();
    if !authorized {
        return Err(failure(
            "AUTHORIZATION_REQUIRED",
            "Confirm that you are authorized to send this request.",
            None,
        ));
    }
    if state.closing.load(Ordering::Acquire) {
        return Err(failure(
            "APP_EXITING",
            "The application is shutting down.",
            None,
        ));
    }
    if state.manager.current().await.is_some_and(|snapshot| {
        matches!(
            snapshot.state,
            RunState::Starting | RunState::Running | RunState::Stopping
        )
    }) {
        return Err(failure(
            "RUN_ALREADY_ACTIVE",
            "Stop the active run before previewing a request.",
            None,
        ));
    }
    let preparation = Preparation::new(&state)?;
    let (config, secrets) = resolve_native(
        &history,
        definition,
        environment,
        rows.unwrap_or_default(),
        preparation.cancelled.clone(),
    )
    .await?;
    if preparation.cancelled.load(Ordering::Acquire) {
        return Err(failure(
            "PREPARATION_CANCELLED",
            "Prepared data was discarded.",
            Some("dataset"),
        ));
    }
    tokio::select! {
        biased;
        _ = exiting => Err(failure("APP_EXITING", "Preview was cancelled because the application is closing.", None)),
        result = velo_engine::preview_request(config, &secrets) => result,
    }
}

#[tauri::command]
pub(crate) async fn save_definition<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
    source: String,
) -> Result<(), RunError> {
    let definition = TestDefinition::parse(&source)?;
    let source = serde_json::to_string(&definition).map_err(|_| {
        failure(
            "AUTHORING_FAILED",
            "The definition could not be encoded.",
            None,
        )
    })?;
    if name.trim().is_empty() || name.len() > 80 {
        return Err(failure(
            "INVALID_DEFINITION_NAME",
            "Enter a name between 1 and 80 bytes.",
            Some("name"),
        ));
    }
    app.state::<History>()
        .call(move |store| store.save_definition(&name, &source))
        .await
}

#[tauri::command]
pub(crate) async fn list_definitions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, RunError> {
    app.state::<History>()
        .call(|store| store.definition_names())
        .await
}

#[tauri::command]
pub(crate) async fn load_definition<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
) -> Result<TestDefinition, RunError> {
    app.state::<History>()
        .call(move |store| TestDefinition::parse(&store.definition(&name)?))
        .await
}

#[tauri::command]
pub(crate) async fn delete_definition<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
) -> Result<(), RunError> {
    app.state::<History>()
        .call(move |store| store.delete_definition(&name))
        .await
}

#[tauri::command]
pub(crate) async fn import_definition_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<TestDefinition>, RunError> {
    use tauri_plugin_dialog::DialogExt;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Velo test", &["json"])
        .pick_file(move |path| {
            let _ = send.send(path);
        });
    let Some(path) = receive
        .await
        .map_err(|_| failure("FILE_DIALOG_FAILED", "File selection failed.", None))?
    else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|_| failure("FILE_PATH_UNSUPPORTED", "Choose a local file.", None))?;
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(|_| {
            failure(
                "FILE_READ_FAILED",
                "The selected file could not be read.",
                None,
            )
        })?;
        let mut source = String::new();
        file.take(8_388_609)
            .read_to_string(&mut source)
            .map_err(|_| failure("FILE_READ_FAILED", "Choose a UTF-8 JSON file.", None))?;
        let mut definition = TestDefinition::parse(&source)?;
        unbind_uploads(&mut definition);
        Ok(Some(definition))
    })
    .await
    .map_err(|_| failure("AUTHORING_FAILED", "File import failed.", None))?
}

fn unbind_uploads(definition: &mut TestDefinition) {
    if let velo_engine::RequestBody::Multipart(parts) = &mut definition.config.body {
        for part in parts {
            if let velo_engine::MultipartPart::File { path, .. } = part {
                path.clear();
            }
        }
    }
}

#[tauri::command]
pub(crate) async fn export_definition_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    source: String,
) -> Result<bool, RunError> {
    use tauri_plugin_dialog::DialogExt;
    let mut definition = TestDefinition::parse(&source)?;
    unbind_uploads(&mut definition);
    let source = serde_json::to_string_pretty(&definition)
        .map_err(|_| failure("AUTHORING_FAILED", "Definition encoding failed.", None))?;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Velo test", &["json"])
        .set_file_name("velo.test.json")
        .save_file(move |path| {
            let _ = send.send(path);
        });
    let Some(path) = receive
        .await
        .map_err(|_| failure("FILE_DIALOG_FAILED", "File selection failed.", None))?
    else {
        return Ok(false);
    };
    let path = path
        .into_path()
        .map_err(|_| failure("FILE_PATH_UNSUPPORTED", "Choose a local file.", None))?;
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Write;
        let parent = path
            .parent()
            .ok_or_else(|| failure("FILE_PATH_UNSUPPORTED", "Choose a local destination.", None))?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| {
            failure(
                "FILE_WRITE_FAILED",
                "The export could not be created.",
                None,
            )
        })?;
        file.write_all(source.as_bytes())
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| {
                failure(
                    "FILE_WRITE_FAILED",
                    "The export could not be written.",
                    None,
                )
            })?;
        file.persist(path)
            .map_err(|_| failure("FILE_WRITE_FAILED", "The export could not be saved.", None))?;
        Ok(true)
    })
    .await
    .map_err(|_| failure("AUTHORING_FAILED", "File export failed.", None))?
}

#[cfg(test)]
mod tests {
    #[test]
    fn preparation_guard_cancels_and_releases_owned_slot() {
        let runs = crate::execution::DesktopRuns::default();
        let preparation = super::Preparation::new(&runs).unwrap();
        let cancelled = preparation.cancelled.clone();
        assert_eq!(
            super::Preparation::new(&runs).err().unwrap().code,
            "PREPARATION_BUSY"
        );
        drop(preparation);
        assert!(cancelled.load(std::sync::atomic::Ordering::Acquire));
        let next = super::Preparation::new(&runs).unwrap();
        assert!(!next.cancelled.load(std::sync::atomic::Ordering::Acquire));
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn remembered_fields_become_references_and_uploads_require_rebinding() {
        let original = json!({ "kind":"multipart", "value":[{"kind":"file", "name":"PRIVATE_NAME", "path":"/PRIVATE_PATH", "filename":"private.bin", "content_type":"application/octet-stream"}] });
        let mut referenced = original.clone();
        reference_values(&mut referenced, "configuration-id", "-body");
        assert_eq!(referenced["kind"], "multipart");
        assert_eq!(referenced["value"][0]["path"], "");
        assert!(
            referenced["value"][0]["name"]
                .as_str()
                .unwrap()
                .starts_with("{{secret:saved.configuration-id.")
        );
        let encoded = referenced.to_string();
        assert!(!encoded.contains("PRIVATE_NAME"));
        assert!(!encoded.contains("PRIVATE_PATH"));
        let mut definition = TestDefinition::default();
        definition.config.body = serde_json::from_value(original).unwrap();
        unbind_uploads(&mut definition);
        let error = definition.resolve(None, |_| unreachable!()).unwrap_err();
        assert_eq!(error.code, "UPLOAD_REBIND_REQUIRED");
    }

    #[test]
    fn definitions_persist_without_resolving_references_and_replace_by_name() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tests.sqlite3");
        let mut definition = TestDefinition::default();
        definition
            .variables
            .insert("token".into(), "{{secret:test-token}}".into());
        let source = serde_json::to_string(&definition).unwrap();
        {
            let store = crate::storage::Store::open(&path).unwrap();
            store.save_definition("test", &source).unwrap();
            store.save_definition("test", &source).unwrap();
            assert_eq!(store.definition_names().unwrap(), vec!["test"]);
        }
        let store = crate::storage::Store::open(&path).unwrap();
        let restored = TestDefinition::parse(&store.definition("test").unwrap()).unwrap();
        assert_eq!(restored.variables["token"], "{{secret:test-token}}");
        store.delete_definition("test").unwrap();
        assert!(store.definition_names().unwrap().is_empty());
    }
}

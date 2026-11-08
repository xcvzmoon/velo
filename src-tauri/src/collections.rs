use tauri_plugin_dialog::DialogExt;
use velo_engine::{ApiCollection, CollectionRequest, RunError, TestDefinition};

use crate::history::History;

fn failure(code: &str, message: &str) -> RunError {
    RunError {
        code: code.into(),
        message: message.into(),
        field: None,
    }
}

#[tauri::command]
pub(crate) async fn save_api_collection(
    state: tauri::State<'_, History>,
    collection: ApiCollection,
) -> Result<(), RunError> {
    collection.validate()?;
    state
        .call(move |store| store.save_api_collection(&collection))
        .await
}

#[tauri::command]
pub(crate) async fn list_api_collections(
    state: tauri::State<'_, History>,
) -> Result<Vec<ApiCollection>, RunError> {
    state.call(|store| store.api_collections()).await
}

#[tauri::command]
pub(crate) async fn reorder_api_collections(
    state: tauri::State<'_, History>,
    ids: Vec<String>,
) -> Result<(), RunError> {
    state
        .call(move |store| store.reorder_api_collections(&ids))
        .await
}

#[tauri::command]
pub(crate) async fn delete_api_collection(
    state: tauri::State<'_, History>,
    sessions: tauri::State<'_, crate::api::ApiState>,
    runs: tauri::State<'_, std::sync::Arc<crate::execution::DesktopRuns>>,
    id: String,
) -> Result<(), RunError> {
    let _admission = runs.admission.try_lock().map_err(|_| {
        failure(
            "REQUEST_BUSY",
            "Wait for the active request operation before deleting its collection.",
        )
    })?;
    let collection_id = id.clone();
    state
        .call(move |store| store.delete_api_collection(&id))
        .await?;
    sessions.clear_collection(&collection_id).await;
    Ok(())
}

#[tauri::command]
pub(crate) async fn collection_request(
    collection: ApiCollection,
    id: String,
) -> Result<CollectionRequest, RunError> {
    collection.request(&id)
}

#[tauri::command]
pub(crate) async fn collection_scenario(
    collection: ApiCollection,
    selection: Vec<String>,
) -> Result<TestDefinition, RunError> {
    collection.scenario(&selection)
}

#[tauri::command]
pub(crate) async fn parse_api_collection(source: String) -> Result<ApiCollection, RunError> {
    ApiCollection::parse(&source)
}

#[tauri::command]
pub(crate) async fn import_api_collection_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<ApiCollection>, RunError> {
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Velo collection", &["json"])
        .pick_file(move |path| {
            let _ = send.send(path);
        });
    let Some(path) = receive
        .await
        .map_err(|_| failure("FILE_DIALOG_FAILED", "File selection failed."))?
    else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|_| failure("FILE_PATH_UNSUPPORTED", "Choose a local file."))?;
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Read;
        let mut source = String::new();
        std::fs::File::open(path)
            .map_err(|_| failure("FILE_READ_FAILED", "The selected file could not be read."))?
            .take(8_388_609)
            .read_to_string(&mut source)
            .map_err(|_| failure("FILE_READ_FAILED", "Choose a UTF-8 JSON file."))?;
        ApiCollection::parse(&source).map(Some)
    })
    .await
    .map_err(|_| {
        failure(
            "COLLECTION_IMPORT_FAILED",
            "Collection import could not complete.",
        )
    })?
}

#[tauri::command]
pub(crate) async fn export_api_collection_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    collection: ApiCollection,
) -> Result<bool, RunError> {
    let portable = collection.portable()?;
    let source = serde_json::to_string_pretty(&portable)
        .map_err(|_| failure("COLLECTION_EXPORT_FAILED", "Collection encoding failed."))?;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Velo collection", &["json"])
        .set_file_name("velo.collection.json")
        .save_file(move |path| {
            let _ = send.send(path);
        });
    let Some(path) = receive
        .await
        .map_err(|_| failure("FILE_DIALOG_FAILED", "File selection failed."))?
    else {
        return Ok(false);
    };
    let path = path
        .into_path()
        .map_err(|_| failure("FILE_PATH_UNSUPPORTED", "Choose a local destination."))?;
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Write;
        let parent = path
            .parent()
            .ok_or_else(|| failure("FILE_PATH_UNSUPPORTED", "Choose a local destination."))?;
        let mut file = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| failure("FILE_WRITE_FAILED", "The export could not be created."))?;
        file.write_all(source.as_bytes())
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| failure("FILE_WRITE_FAILED", "The export could not be written."))?;
        file.persist(path)
            .map_err(|_| failure("FILE_WRITE_FAILED", "The export could not be saved."))?;
        Ok(true)
    })
    .await
    .map_err(|_| {
        failure(
            "COLLECTION_EXPORT_FAILED",
            "Collection export could not complete.",
        )
    })?
}

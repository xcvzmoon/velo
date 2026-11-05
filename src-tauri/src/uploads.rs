use tauri_plugin_dialog::DialogExt;
use velo_engine::RunError;

#[tauri::command]
pub(crate) async fn select_upload_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, RunError> {
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_file(move |path| {
        let _ = send.send(path);
    });
    let path = receive.await.map_err(|_| RunError {
        code: "FILE_DIALOG_FAILED".into(),
        message: "The file selector could not complete.".into(),
        field: None,
    })?;
    path.map(|path| {
        path.into_path()
            .ok()
            .and_then(|path| path.into_os_string().into_string().ok())
            .ok_or_else(|| RunError {
                code: "UPLOAD_PATH_UNSUPPORTED".into(),
                message: "Choose a local file with a UTF-8 path.".into(),
                field: Some("body".into()),
            })
    })
    .transpose()
}

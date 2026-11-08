use tauri_plugin_dialog::DialogExt;
use velo_contracts::reports::{ComparisonReport, ComparisonSettings, RunMetadata};
use velo_engine::RunError;

use crate::history::History;

fn input_failure(error: velo_contracts::CommandError) -> RunError {
    RunError {
        code: error.code,
        message: error.message,
        field: error.field,
    }
}

#[tauri::command]
pub(crate) async fn pinned_baseline(
    state: tauri::State<'_, History>,
) -> Result<Option<String>, RunError> {
    state.call(|store| store.baseline()).await
}

#[tauri::command]
pub(crate) async fn pin_baseline(
    state: tauri::State<'_, History>,
    id: Option<String>,
) -> Result<(), RunError> {
    state
        .call(move |store| store.pin_baseline(id.as_deref()))
        .await
}

#[tauri::command]
pub(crate) async fn update_run_metadata(
    state: tauri::State<'_, History>,
    id: String,
    metadata: RunMetadata,
) -> Result<(), RunError> {
    velo_contracts::reports::validate_metadata(&metadata).map_err(input_failure)?;
    state
        .call(move |store| store.update_metadata(&id, &metadata))
        .await
}

#[tauri::command]
pub(crate) async fn load_comparison_settings(
    state: tauri::State<'_, History>,
    id: String,
) -> Result<Option<ComparisonSettings>, RunError> {
    state
        .call(move |store| store.comparison_settings(&id))
        .await
}

#[tauri::command]
pub(crate) async fn save_comparison(
    state: tauri::State<'_, History>,
    id: String,
    settings: ComparisonSettings,
) -> Result<ComparisonReport, RunError> {
    velo_contracts::reports::validate_settings(&settings).map_err(input_failure)?;
    state
        .call(move |store| {
            let report = store.comparison_report(&id, &settings)?;
            store.save_comparison(&id, &settings, &report)?;
            Ok(report)
        })
        .await
}

fn export_failure() -> RunError {
    RunError {
        code: "REPORT_EXPORT_FAILED".into(),
        message:
            "The report could not be saved. Check destination permissions and available space."
                .into(),
        field: None,
    }
}

fn export_io_failure(error: std::io::Error) -> RunError {
    tracing::warn!(code = "REPORT_EXPORT_FAILED", kind = ?error.kind(), "Report destination write failed");
    export_failure()
}

#[tauri::command]
pub(crate) async fn export_comparison<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, History>,
    id: String,
    format: String,
) -> Result<bool, RunError> {
    let extension = match format.as_str() {
        "html" => "html",
        "json" => "json",
        "junit" => "xml",
        _ => {
            return Err(RunError {
                code: "INVALID_REPORT_FORMAT".into(),
                message: "Choose HTML, JSON, or JUnit.".into(),
                field: Some("format".into()),
            });
        }
    };
    let content = state
        .call(move |store| {
            let settings = store.comparison_settings(&id)?.ok_or_else(|| RunError {
                code: "COMPARISON_UNAVAILABLE".into(),
                message: "Save comparison settings before exporting.".into(),
                field: None,
            })?;
            let report = store.comparison_report(&id, &settings)?;
            match format.as_str() {
                "html" => velo_contracts::reports::export_html(&report).map_err(input_failure),
                "json" => velo_contracts::reports::export_json(&report).map_err(input_failure),
                _ => velo_contracts::reports::export_junit(&report).map_err(input_failure),
            }
        })
        .await?;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(format!("velo-report.{extension}"))
        .add_filter("Velo report", &[extension])
        .save_file(move |path| {
            let _ = send.send(path);
        });
    let Some(path) = receive.await.map_err(|_| export_failure())? else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|_| export_failure())?;
    tauri::async_runtime::spawn_blocking(move || write_report(&path, &content).map(|()| true))
        .await
        .map_err(|_| export_failure())?
}

fn write_report(path: &std::path::Path, content: &str) -> Result<(), RunError> {
    use std::io::Write;
    let parent = path.parent().ok_or_else(export_failure)?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(export_io_failure)?;
    file.write_all(content.as_bytes())
        .map_err(export_io_failure)?;
    file.as_file().sync_all().map_err(export_io_failure)?;
    file.persist(path)
        .map_err(|error| export_io_failure(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_writes_replace_complete_files_and_clean_up_failed_destinations() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        std::fs::write(&path, "original").unwrap();
        write_report(&path, "{\"format_version\":1}").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"format_version\":1}"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        let blocked = directory.path().join("blocked");
        std::fs::create_dir(&blocked).unwrap();
        let error = write_report(&blocked, "cannot replace directory").unwrap_err();
        assert_eq!(error.code, "REPORT_EXPORT_FAILED");
        assert!(!error.message.contains(directory.path().to_str().unwrap()));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
        let missing = directory.path().join("missing").join("report.json");
        assert_eq!(
            write_report(&missing, "data").unwrap_err().code,
            "REPORT_EXPORT_FAILED"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}

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

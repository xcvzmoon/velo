mod authoring;
mod credentials;
mod diagnostic;
mod execution;
mod history;
#[cfg(target_os = "macos")]
mod macos_exit;
mod panic_policy;
mod reports;
mod saved_requests;
mod storage;
mod uploads;

use std::sync::{Arc, atomic::Ordering};

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use velo_engine::RunState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Launches the desktop shell, diagnostics, and native run commands.
///
/// Active runs require exit confirmation. Confirmed exit closes admission and waits
/// for engine cleanup and aggregate publisher teardown before runtime shutdown.
///
/// # Returns
/// Returns after the application exits and native run cleanup completes.
///
/// # Examples
/// ```no_run
/// app_lib::run();
/// ```
///
/// # Abort
/// Aborts on any programming panic, including desktop initialization failure.
/// Panic payloads are omitted from diagnostics to protect request data.
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
    panic_policy::install();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(execution::DesktopRuns::default()))
        .invoke_handler(tauri::generate_handler![
            authoring::import_definition_file,
            authoring::export_definition_file,
            authoring::load_configuration_definition,
            authoring::parse_definition,
            authoring::resolve_definition,
            authoring::cancel_data_preparation,
            authoring::import_requests,
            authoring::set_authoring_secret,
            authoring::preview_request,
            authoring::save_definition,
            authoring::list_definitions,
            authoring::load_definition,
            authoring::delete_definition,
            reports::pinned_baseline,
            reports::pin_baseline,
            reports::update_run_metadata,
            reports::load_comparison_settings,
            reports::save_comparison,
            history::history_status,
            history::list_history,
            history::load_history,
            history::delete_history,
            history::list_configurations,
            history::load_configuration,
            history::delete_configuration,
            diagnostic::diagnose,
            execution::prepare_run,
            execution::start_run,
            execution::stop_run,
            execution::current_run,
            uploads::select_upload_file,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<Arc<execution::DesktopRuns>>();
                if !state.exit_ready.load(Ordering::Acquire) {
                    api.prevent_close();
                    window.app_handle().exit(0);
                }
            }
        });
    let builder = builder.setup(|app| {
        history::initialize(app.handle());
        #[cfg(target_os = "macos")]
        macos_exit::install(app.handle())?;
        Ok(())
    });
    builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                let state = app.state::<Arc<execution::DesktopRuns>>().inner().clone();
                if state.exit_ready.load(Ordering::Acquire) {
                    return;
                }
                api.prevent_exit();
                if !state.closing.swap(true, Ordering::AcqRel) {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        state.preview_exit.notify_waiters();
                        if let Ok(current) = state.preparation.lock()
                            && let Some(cancelled) = current.as_ref()
                        {
                            cancelled.store(true, Ordering::Release);
                        }
                        let _admission = state.admission.lock().await;
                        let active = state.manager.current().await.is_some_and(|snapshot| {
                            matches!(snapshot.state, RunState::Starting | RunState::Running | RunState::Stopping)
                        });
                        tracing::info!(active_run = active, "Exit requested");
                        if active {
                            let dialog_app = app.clone();
                            let confirmed = tauri::async_runtime::spawn_blocking(move || {
                                let Some(window) = dialog_app.get_webview_window("main") else {
                                    tracing::error!(code = "EXIT_WINDOW_UNAVAILABLE", "Exit confirmation window is unavailable");
                                    return false;
                                };
                                dialog_app.dialog()
                                    .message("A load test is active. Stop the run and quit? Partial results will be retained when local storage is available.")
                                    .title("Quit Velo?")
                                    .parent(&window)
                                    .kind(MessageDialogKind::Warning)
                                    .buttons(MessageDialogButtons::OkCancelCustom("Stop and quit".into(), "Keep running".into()))
                                    .blocking_show()
                            }).await.unwrap_or(false);
                            if !confirmed {
                                tracing::info!("Exit cancelled");
                                state.closing.store(false, Ordering::Release);
                                return;
                            }
                        }
                        tracing::info!("Awaiting native run cleanup before exit");
                        state.manager.shutdown().await;
                        if let Some(monitor) = state.monitor.lock().await.take() {
                            let _ = monitor.await;
                        }
                        state.exit_ready.store(true, Ordering::Release);
                        app.exit(code.unwrap_or(0));
                    });
                }
            }
        });
}

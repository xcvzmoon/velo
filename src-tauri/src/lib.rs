mod diagnostic;
mod execution;

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
/// # Panics
/// Panics if the desktop runtime cannot initialize.
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(execution::DesktopRuns::default()))
        .invoke_handler(tauri::generate_handler![
            diagnostic::diagnose,
            execution::prepare_run,
            execution::start_run,
            execution::stop_run,
            execution::current_run,
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
                                    .message("A load test is active. Stop the run and quit? Partial results will be lost when Velo closes.")
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
                            monitor.abort();
                            let _ = monitor.await;
                        }
                        state.exit_ready.store(true, Ordering::Release);
                        app.exit(code.unwrap_or(0));
                    });
                }
            }
        });
}

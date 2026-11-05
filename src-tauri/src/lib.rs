mod diagnostic;
mod execution;

use std::sync::{Arc, atomic::Ordering};

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Launches the desktop shell, diagnostics, and native run commands.
///
/// Exit closes run admission and waits for engine cleanup before runtime teardown.
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
    tauri::Builder::default()
        .manage(Arc::new(execution::DesktopRuns::default()))
        .invoke_handler(tauri::generate_handler![
            diagnostic::diagnose,
            execution::prepare_run,
            execution::start_run,
            execution::stop_run,
            execution::current_run,
        ])
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
                        state.manager.shutdown().await;
                        state.exit_ready.store(true, Ordering::Release);
                        app.exit(code.unwrap_or(0));
                    });
                }
            }
        });
}

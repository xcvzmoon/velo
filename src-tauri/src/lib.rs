mod diagnostic;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Launches the desktop shell and registers native diagnostics.
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
        .invoke_handler(tauri::generate_handler![diagnostic::diagnose])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}

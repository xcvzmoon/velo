use std::{io, sync::OnceLock};

use objc2::{
    MainThreadMarker,
    runtime::{AnyObject, Imp, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSApplicationTerminateReply};

static APPLICATION: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Routes AppKit menu, keyboard, and Dock termination through native run cleanup.
/// Installation runs once on the main thread, before the application accepts runs.
pub(crate) fn install(app: &tauri::AppHandle) -> Result<(), io::Error> {
    let main_thread =
        MainThreadMarker::new().ok_or_else(|| io::Error::other("MACOS_EXIT_HANDLER_THREAD"))?;
    let application = NSApplication::sharedApplication(main_thread);
    let delegate = application
        .delegate()
        .ok_or_else(|| io::Error::other("MACOS_EXIT_DELEGATE_MISSING"))?;
    let delegate_object: &AnyObject = (*delegate).as_ref();
    let selector = sel!(applicationShouldTerminate:);
    if delegate_object.class().instance_method(selector).is_some() {
        return Err(io::Error::other("MACOS_EXIT_HANDLER_CONFLICT"));
    }
    APPLICATION
        .set(app.clone())
        .map_err(|_| io::Error::other("MACOS_EXIT_HANDLER_ALREADY_INSTALLED"))?;
    // SAFETY: The callback matches AppKit's NSUInteger-returning delegate method.
    // The existing delegate layout is unchanged; installation occurs once on the main thread.
    let installed = unsafe {
        let implementation = std::mem::transmute::<
            extern "C-unwind" fn(&AnyObject, Sel, &NSApplication) -> NSApplicationTerminateReply,
            Imp,
        >(should_terminate);
        objc2::ffi::class_addMethod(
            std::ptr::from_ref(delegate_object.class()).cast_mut(),
            selector,
            implementation,
            c"Q@:@".as_ptr(),
        )
    };
    if !installed.as_bool() {
        return Err(io::Error::other("MACOS_EXIT_HANDLER_INSTALL_FAILED"));
    }
    application.setDelegate(Some(&delegate));
    Ok(())
}

extern "C-unwind" fn should_terminate(
    _delegate: &AnyObject,
    _selector: Sel,
    _application: &NSApplication,
) -> NSApplicationTerminateReply {
    if let Some(app) = APPLICATION.get() {
        tracing::info!("AppKit exit requested");
        app.exit(0);
    }
    NSApplicationTerminateReply::TerminateCancel
}

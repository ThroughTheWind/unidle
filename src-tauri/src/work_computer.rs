//! The work computer notice (SPEC §16): once, at the first launch that shows
//! the window, a local read-only check for signs that an organization
//! manages this computer. Finding none shows nothing: it is no reassurance.

use tauri::{AppHandle, Manager};

use crate::{App, settings};

/// The reason the check found, such as `"mdm"`, or `None`. Only the first
/// call ever runs the check; later calls return `None` at once. Async, so
/// the check (a few registry reads, or `profiles` on macOS) stays off the
/// main thread.
#[tauri::command]
pub async fn take_managed_notice(app: AppHandle) -> Option<&'static str> {
    if app.state::<App>().settings().managed_check_done {
        return None;
    }
    settings::record(&app, |settings| settings.managed_check_done = true);
    unidle_core::managed_signal().map(unidle_core::ManagedSignal::name)
}

/// The same check, on request from Settings.
#[tauri::command]
pub async fn check_managed() -> Option<&'static str> {
    unidle_core::managed_signal().map(unidle_core::ManagedSignal::name)
}

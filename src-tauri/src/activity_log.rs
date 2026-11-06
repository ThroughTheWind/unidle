//! The activity log file (SPEC §15): sessions go to `activity-log.csv` in the
//! app's data directory while the setting is on. It never leaves the computer
//! unless the user exports it.

use std::path::PathBuf;

use jiff::tz::TimeZone;
use tauri::{AppHandle, Manager};
use unidle_core::activity;

use crate::{App, strings};

const FILE_NAME: &str = "activity-log.csv";

/// `activity-log.csv` in the platform's data directory, for example
/// `%APPDATA%\com.throughthewind.unidle` on Windows.
pub fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(FILE_NAME))
}

/// Sends finished sessions to the file while `enabled`, and nowhere
/// otherwise. Called with the settings at startup and after each change.
pub fn apply(app: &AppHandle, enabled: bool) {
    let sink: Option<unidle_core::SessionSink> = match (enabled, path(app)) {
        (true, Some(path)) => Some(Box::new(move |session| {
            if let Err(error) = activity::append(&path, &session, &TimeZone::system()) {
                eprintln!("activity log: {error}");
            }
        })),
        _ => None,
    };
    app.state::<App>().controller().set_session_sink(sink);
}

/// Saves a copy of the log where the user picks. `false` when they cancel.
/// Async so the dialog doesn't block the main thread it needs.
#[tauri::command]
pub async fn export_activity_log(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt as _;
    let source = path(&app).ok_or("No data directory")?;
    let Some(target) = app
        .dialog()
        .file()
        .set_file_name(strings::ACTIVITY_FILE_NAME)
        .add_filter(strings::ACTIVITY_FILE_FILTER, &["csv"])
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let target = target.into_path().map_err(|error| error.to_string())?;
    let text = activity::read(&source).map_err(|error| error.to_string())?;
    std::fs::write(target, text).map_err(|error| error.to_string())?;
    Ok(true)
}

/// Deletes every logged session. Turning the log off keeps them.
#[tauri::command]
pub fn clear_activity_log(app: AppHandle) -> Result<(), String> {
    let path = path(&app).ok_or("No data directory")?;
    activity::clear(&path).map_err(|error| error.to_string())
}

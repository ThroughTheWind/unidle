//! The main window's lifecycle (SPEC FR-12, NFR-1).
//!
//! Closing the window destroys it, webview included, instead of hiding it:
//! the webview is most of the app's memory, and the memory limit applies
//! while the app sits in the tray (F-002). Opening it again builds a fresh
//! one from the window config.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;

use crate::strings;

pub const MAIN: &str = "main";
/// Exists once the "still running in the tray" notice has been shown.
const NOTICE_MARKER: &str = "tray-notice-shown";

pub fn show(app: &AppHandle) {
    let window = match app.get_webview_window(MAIN) {
        Some(window) => Some(window),
        None => build(app),
    };
    if let Some(window) = window {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build(app: &AppHandle) -> Option<WebviewWindow> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN)?
        .clone();
    WebviewWindowBuilder::from_config(app, &config)
        .and_then(|builder| builder.build())
        .ok()
}

/// Called when the main window has been destroyed by closing it. The first
/// time ever, says that the app is still running (SPEC FR-12).
pub fn on_closed(app: &AppHandle) {
    let Some(marker) = notice_marker(app) else {
        return;
    };
    if marker.exists() {
        return;
    }
    let shown = app
        .notification()
        .builder()
        .title(strings::STILL_RUNNING)
        .body(strings::STILL_RUNNING_BODY)
        .show();
    if shown.is_ok() {
        if let Some(dir) = marker.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&marker, b"");
    }
}

fn notice_marker(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(NOTICE_MARKER))
}

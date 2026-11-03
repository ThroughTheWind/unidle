// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use unidle_core::{Controller, InputUnavailable, Source, Status, SystemAdapter, Ticker};

struct App {
    controller: Arc<Mutex<Controller<SystemAdapter>>>,
    ticker: Ticker,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusDto {
    on: bool,
    elapsed_secs: u64,
    power_assertion: bool,
    /// Nudges are being sent: ON, enabled, and possible on this OS.
    nudging: bool,
    /// Why nudges cannot be sent, if they cannot (SPEC PB-2).
    input_unavailable: Option<&'static str>,
}

impl From<Status> for StatusDto {
    fn from(status: Status) -> Self {
        Self {
            on: status.is_on(),
            elapsed_secs: status.elapsed.as_secs(),
            power_assertion: status.capabilities.power_assertion,
            nudging: status.nudging,
            input_unavailable: status
                .capabilities
                .input_unavailable
                .map(|reason| match reason {
                    InputUnavailable::NeedsPermission => "needsPermission",
                    InputUnavailable::Wayland => "wayland",
                    InputUnavailable::NoDisplay => "noDisplay",
                    InputUnavailable::Unsupported => "unsupported",
                }),
        }
    }
}

#[tauri::command]
fn get_status(app: State<'_, App>) -> StatusDto {
    app.controller.lock().unwrap().status().into()
}

#[tauri::command]
fn set_active(on: bool, app: State<'_, App>) -> Result<StatusDto, String> {
    let mut controller = app.controller.lock().unwrap();
    let result = if on {
        let wants_nudges = controller.nudge_config().enabled;
        let result = controller.turn_on(Source::Manual);
        // macOS lists the app under Accessibility only once it asks, so ask
        // the first time nudges are wanted and the permission is missing.
        if result.is_ok()
            && wants_nudges
            && controller.status().capabilities.input_unavailable
                == Some(InputUnavailable::NeedsPermission)
        {
            unidle_core::request_input_permission();
        }
        result
    } else {
        controller.turn_off()
    };
    app.ticker.wake();
    result.map_err(|error| error.to_string())?;
    Ok(controller.status().into())
}

#[tauri::command]
fn open_input_permission_settings() -> Result<(), String> {
    unidle_core::open_input_permission_settings().map_err(|error| error.to_string())
}

fn main() {
    let controller = Arc::new(Mutex::new(Controller::new(SystemAdapter::default())));
    let ticker = Ticker::spawn(controller.clone());

    tauri::Builder::default()
        .manage(App { controller, ticker })
        .invoke_handler(tauri::generate_handler![
            get_status,
            set_active,
            open_input_permission_settings
        ])
        .run(tauri::generate_context!())
        .expect("failed to run the application");
}

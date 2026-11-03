// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use serde::Serialize;
use tauri::State;
use unidle_core::{Controller, Source, Status, SystemAdapter};

type AppController = Mutex<Controller<SystemAdapter>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusDto {
    on: bool,
    elapsed_secs: u64,
    power_assertion: bool,
    input_nudge: bool,
}

impl From<Status> for StatusDto {
    fn from(status: Status) -> Self {
        Self {
            on: status.is_on(),
            elapsed_secs: status.elapsed.as_secs(),
            power_assertion: status.capabilities.power_assertion,
            input_nudge: status.capabilities.input_nudge,
        }
    }
}

#[tauri::command]
fn get_status(controller: State<'_, AppController>) -> StatusDto {
    controller.lock().unwrap().status().into()
}

#[tauri::command]
fn set_active(on: bool, controller: State<'_, AppController>) -> Result<StatusDto, String> {
    let mut controller = controller.lock().unwrap();
    let result = if on {
        controller.turn_on(Source::Manual)
    } else {
        controller.turn_off()
    };
    result.map_err(|error| error.to_string())?;
    Ok(controller.status().into())
}

fn main() {
    tauri::Builder::default()
        .manage(Mutex::new(Controller::new(SystemAdapter::default())))
        .invoke_handler(tauri::generate_handler![get_status, set_active])
        .run(tauri::generate_context!())
        .expect("failed to run the application");
}

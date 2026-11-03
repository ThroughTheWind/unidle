// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;
mod strings;
mod tray;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;
use tauri_plugin_notification::NotificationExt as _;
use unidle_core::{
    Controller, InputUnavailable, NudgeKey, Settings, Source, Status, SystemAdapter, Ticker,
};

/// Emitted to the window whenever the state changes outside it, such as
/// from the tray.
const STATUS_EVENT: &str = "status-changed";
/// How often the tray's "ON for …" line is refreshed.
const TRAY_REFRESH: Duration = Duration::from_secs(30);
/// Passed by the login item, so a launch at login starts in the tray.
const HIDDEN_ARG: &str = "--hidden";

struct App {
    controller: Arc<Mutex<Controller<SystemAdapter>>>,
    ticker: Ticker,
    /// Set by an explicit quit, so the app exits instead of staying in the tray.
    quitting: AtomicBool,
    settings: Mutex<Settings>,
}

impl App {
    fn controller(&self) -> MutexGuard<'_, Controller<SystemAdapter>> {
        // A poisoned lock means a panic mid-update; the state itself is still
        // the best record of whether the assertion is held.
        self.controller
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn settings(&self) -> MutexGuard<'_, Settings> {
        self.settings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Clone, Serialize)]
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

/// Turns simulation ON or OFF from any surface, then updates every surface.
fn set_active(app: &AppHandle, on: bool) -> Result<Status, String> {
    let state = app.state::<App>();
    let (result, status) = {
        let mut controller = state.controller();
        let result = if on {
            let wants_nudges = controller.nudge_config().enabled;
            let result = controller.turn_on(Source::Manual);
            // macOS lists the app under Accessibility only once it asks, so
            // ask the first time nudges are wanted and the permission is
            // missing.
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
        (result, controller.status())
    };
    state.ticker.wake();
    tray::refresh(app, &status);
    let _ = app.emit(STATUS_EVENT, StatusDto::from(status));
    result.map_err(|error| error.to_string())?;

    let notify = state.settings().notify_on_change;
    if state.settings().last_state_on != status.is_on() {
        settings::record(app, |settings| settings.last_state_on = status.is_on());
    }
    // Optional, off by default (SPEC FR-16).
    if notify {
        let _ = app
            .notification()
            .builder()
            .title(if status.is_on() {
                strings::TOOLTIP_ON
            } else {
                strings::TOOLTIP_OFF
            })
            .show();
    }
    Ok(status)
}

pub(crate) fn toggle(app: &AppHandle) {
    let on = app.state::<App>().controller().status().is_on();
    let _ = set_active(app, !on);
}

/// Quit from the tray or app menu: OFF first, then exit (SPEC FR-13).
pub(crate) fn quit(app: &AppHandle) {
    let state = app.state::<App>();
    state.quitting.store(true, Ordering::SeqCst);
    let _ = state.controller().turn_off();
    app.exit(0);
}

#[tauri::command]
fn get_status(app: State<'_, App>) -> StatusDto {
    app.controller().status().into()
}

#[tauri::command(rename = "set_active")]
fn set_active_command(on: bool, app: AppHandle) -> Result<StatusDto, String> {
    set_active(&app, on).map(StatusDto::from)
}

#[tauri::command]
fn get_settings(app: State<'_, App>) -> Settings {
    app.settings().clone()
}

/// Takes any subset of the settings fields, in camelCase.
#[tauri::command]
fn update_settings(patch: serde_json::Value, app: AppHandle) -> Result<Settings, String> {
    let settings = settings::update(&app, patch)?;
    let status = app.state::<App>().controller().status();
    tray::refresh(&app, &status);
    let _ = app.emit(STATUS_EVENT, StatusDto::from(status));
    Ok(settings)
}

/// The keys a keystroke nudge may use on this OS, for the settings screen.
#[tauri::command]
fn nudge_keys() -> Vec<&'static str> {
    NudgeKey::ALL
        .into_iter()
        .filter(|key| key.is_safe_here())
        .map(NudgeKey::name)
        .collect()
}

#[tauri::command]
fn open_input_permission_settings() -> Result<(), String> {
    unidle_core::open_input_permission_settings().map_err(|error| error.to_string())
}

/// macOS: an app menu whose Quit goes through `quit`, so Cmd+Q turns OFF
/// first like the tray's Quit.
#[cfg(target_os = "macos")]
fn app_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

    let quit = MenuItem::with_id(
        app,
        "app-quit",
        strings::QUIT_APP,
        true,
        Some("CmdOrCtrl+Q"),
    )?;
    let app_submenu = Submenu::with_items(
        app,
        strings::APP_NAME,
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    let window_submenu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    Menu::with_items(app, &[&app_submenu, &window_submenu])
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();

    #[cfg(target_os = "macos")]
    {
        app.set_menu(app_menu(&handle)?)?;
        app.on_menu_event(|app, event| {
            if event.id().as_ref() == "app-quit" {
                quit(app);
            }
        });
    }

    let menu = tray::create(&handle)?;
    app.manage(menu);

    let loaded = settings::load(&handle);
    *app.state::<App>().settings() = loaded.clone();
    if let Err(error) = settings::apply_to_controller(&handle, &loaded) {
        eprintln!("settings: {error}");
    }
    if let Err(error) = settings::apply_shortcut(&handle, loaded.global_shortcut.as_deref()) {
        eprintln!("settings: {error}");
    }
    if loaded.start_on() {
        let _ = set_active(&handle, true);
    }
    // A launch at login stays in the tray; any other launch shows the window.
    if !std::env::args().any(|arg| arg == HIDDEN_ARG) {
        window::show(&handle);
    }

    // Keeps the tray's "ON for …" line current while ON.
    let refresher = handle.clone();
    std::thread::Builder::new()
        .name("unidle-tray-refresh".into())
        .spawn(move || {
            loop {
                std::thread::sleep(TRAY_REFRESH);
                let status = refresher.state::<App>().controller().status();
                if status.is_on() {
                    tray::refresh(&refresher, &status);
                }
            }
        })?;
    Ok(())
}

fn main() {
    let controller = Arc::new(Mutex::new(Controller::new(SystemAdapter::default())));
    let ticker = Ticker::spawn(controller.clone());

    tauri::Builder::default()
        // First, so a second launch exits before it builds a second tray. It
        // shows the running app's window instead.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            window::show(app)
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![HIDDEN_ARG]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle(app);
                    }
                })
                .build(),
        )
        .manage(App {
            controller,
            ticker,
            quitting: AtomicBool::new(false),
            settings: Mutex::new(Settings::default()),
        })
        .setup(setup)
        .on_window_event(|window, event| {
            if window.label() == window::MAIN && matches!(event, WindowEvent::Destroyed) {
                window::on_closed(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            set_active_command,
            get_settings,
            update_settings,
            nudge_keys,
            open_input_permission_settings
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the application")
        .run(|app, event| match event {
            // The last window closed: stay in the tray (SPEC FR-12). Only an
            // explicit quit exits.
            RunEvent::ExitRequested { api, code, .. } => {
                let quitting = app.state::<App>().quitting.load(Ordering::SeqCst);
                if code.is_none() && !quitting {
                    api.prevent_exit();
                }
            }
            // However the app ends, release before the process goes.
            RunEvent::Exit => {
                let _ = app.state::<App>().controller().turn_off();
            }
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => window::show(app),
            _ => {}
        });
}

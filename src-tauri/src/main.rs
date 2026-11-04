// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_icon;
mod settings;
mod strings;
mod tray;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use jiff::{SignedDuration, Timestamp, Zoned};
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
    /// The tray asked for the timer picker; the window takes it once loaded.
    timer_picker_requested: AtomicBool,
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
    /// `"manual"`, `"timer"` or `"schedule"` while ON (SPEC SC-4).
    source: Option<&'static str>,
    next_change: Option<NextChangeDto>,
    /// Milliseconds since the Unix epoch, while a manual OFF pauses a window.
    paused_until: Option<i64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NextChangeDto {
    /// Milliseconds since the Unix epoch, for `new Date()`.
    at: i64,
    on: bool,
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
            source: status.source.map(|source| match source {
                Source::Manual => "manual",
                Source::Timer => "timer",
                Source::Schedule => "schedule",
            }),
            next_change: status.next_change.map(|change| NextChangeDto {
                at: change.at.as_millisecond(),
                on: change.on,
            }),
            paused_until: status.paused_until.map(Timestamp::as_millisecond),
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
            let result = controller.turn_on();
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

    if state.settings().last_state_on != status.is_on() {
        settings::record(app, |settings| settings.last_state_on = status.is_on());
    }
    notify(app, &status);
    Ok(status)
}

/// The optional notification on each state change, off by default (SPEC
/// FR-16).
fn notify(app: &AppHandle, status: &Status) {
    if !app.state::<App>().settings().notify_on_change {
        return;
    }
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

/// A timer or the schedule changed the state on its own: every surface
/// catches up.
fn on_automatic_change(app: &AppHandle) {
    let status = app.state::<App>().controller().status();
    tray::refresh(app, &status);
    let _ = app.emit(STATUS_EVENT, StatusDto::from(status));
    notify(app, &status);
}

/// How long a quick timer runs (SPEC SC-3).
pub(crate) enum TimerLength {
    Minutes(u32),
    /// Minutes since local midnight: the next time the clock shows it.
    Until(u16),
}

/// The longest timer: a day. Longer is a schedule.
const MAX_TIMER_MINUTES: u32 = 24 * 60;

/// Keeps simulation ON until the timer ends, then the schedule decides.
pub(crate) fn start_timer(app: &AppHandle, length: TimerLength) -> Result<Status, String> {
    let now = Zoned::now();
    let until = match length {
        TimerLength::Minutes(minutes) if (1..=MAX_TIMER_MINUTES).contains(&minutes) => {
            now.timestamp() + SignedDuration::from_mins(i64::from(minutes))
        }
        TimerLength::Minutes(_) => return Err("A timer runs for 1 minute to 24 hours".into()),
        TimerLength::Until(minutes) => unidle_core::next_local_time(&now, minutes)
            .ok_or_else(|| "That time doesn't exist here".to_string())?,
    };
    let state = app.state::<App>();
    let (result, status) = {
        let mut controller = state.controller();
        let result = controller.start_timer_at(until, &now);
        (result, controller.status())
    };
    state.ticker.wake();
    tray::refresh(app, &status);
    let _ = app.emit(STATUS_EVENT, StatusDto::from(status));
    result.map_err(|error| error.to_string())?;
    notify(app, &status);
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

/// `minutes` for "ON for 30 min", or `until` ("18:00") for "until 18:00".
#[tauri::command(rename = "start_timer")]
fn start_timer_command(
    minutes: Option<u32>,
    until: Option<String>,
    app: AppHandle,
) -> Result<StatusDto, String> {
    let length = match (minutes, until) {
        (Some(minutes), None) => TimerLength::Minutes(minutes),
        (None, Some(until)) => TimerLength::Until(
            unidle_core::parse_time(&until)
                .filter(|minutes| *minutes < unidle_core::MINUTES_PER_DAY)
                .ok_or_else(|| format!("Not a time: {until}"))?,
        ),
        _ => return Err("Give minutes or a time".into()),
    };
    start_timer(&app, length).map(StatusDto::from)
}

/// Saves the schedule to a file the user picks. `false` when they cancel.
/// Async so the dialog doesn't block the main thread it needs.
#[tauri::command]
async fn export_schedule(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt as _;
    let Some(path) = app
        .dialog()
        .file()
        .set_file_name(strings::SCHEDULE_FILE_NAME)
        .add_filter(strings::SCHEDULE_FILE_FILTER, &["json"])
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|error| error.to_string())?;
    let file = app.state::<App>().settings().schedule_file();
    std::fs::write(path, file).map_err(|error| error.to_string())?;
    Ok(true)
}

/// Replaces the schedule's times and days off with a file the user picks.
/// `None` when they cancel. The on/off switch stays as it is.
#[tauri::command]
async fn import_schedule(app: AppHandle) -> Result<Option<Settings>, String> {
    use tauri_plugin_dialog::DialogExt as _;
    let Some(path) = app
        .dialog()
        .file()
        .add_filter(strings::SCHEDULE_FILE_FILTER, &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|error| error.to_string())?;
    // A schedule file is small; anything big is something else.
    let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > 1_000_000 {
        return Err("This isn't an Unidle schedule file".into());
    }
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let patch = Settings::schedule_patch_from_file(&text)?;
    let settings = settings::update(&app, patch)?;
    let status = app.state::<App>().controller().status();
    tray::refresh(&app, &status);
    let _ = app.emit(STATUS_EVENT, StatusDto::from(status));
    Ok(Some(settings))
}

/// Whether the tray asked for the timer picker since the last call.
#[tauri::command]
fn take_timer_picker_request(app: State<'_, App>) -> bool {
    app.timer_picker_requested.swap(false, Ordering::SeqCst)
}

#[tauri::command]
fn get_settings(app: State<'_, App>) -> Settings {
    app.settings().clone()
}

/// Takes any subset of the settings fields, in camelCase.
#[tauri::command]
fn update_settings(patch: serde_json::Value, app: AppHandle) -> Result<Settings, String> {
    let settings = settings::update(&app, patch)?;
    app_icon::apply(&app);
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

/// Pages the window links to. The window names one and the URL stays here, so
/// it can't open anything else. The browser loads the page: the app itself
/// makes no network request (SPEC NFR-2).
const RELEASES_URL: &str = "https://github.com/ThroughTheWind/unidle/releases";
const WEBSITE_URL: &str = "https://throughthewind.github.io/unidle/";

#[tauri::command]
fn open_link(app: AppHandle, link: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = match link.as_str() {
        "releases" => RELEASES_URL,
        "website" => WEBSITE_URL,
        _ => return Err(format!("unknown link: {link}")),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())
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
    // The tray was built before the settings were read.
    app_icon::apply(&handle);
    tray::refresh(&handle, &app.state::<App>().controller().status());
    if loaded.start_on() {
        let _ = set_active(&handle, true);
    }
    // A launch at login stays in the tray; any other launch shows the window.
    if !std::env::args().any(|arg| arg == HIDDEN_ARG) {
        window::show(&handle);
    }

    // Keeps the tray's "ON for …" and next change lines current.
    let refresher = handle.clone();
    std::thread::Builder::new()
        .name("unidle-tray-refresh".into())
        .spawn(move || {
            loop {
                std::thread::sleep(TRAY_REFRESH);
                let status = refresher.state::<App>().controller().status();
                // "tomorrow at 09:00" turns into "at 09:00" at midnight.
                if status.is_on() || status.next_change.is_some() {
                    tray::refresh(&refresher, &status);
                }
            }
        })?;
    Ok(())
}

fn main() {
    let controller = Arc::new(Mutex::new(Controller::new(SystemAdapter::default())));
    // The ticker starts before the app exists; it reaches the app once setup
    // has run.
    let handle: Arc<OnceLock<AppHandle>> = Arc::default();
    let ticker_handle = handle.clone();
    let ticker = Ticker::spawn(controller.clone(), move || {
        if let Some(app) = ticker_handle.get() {
            on_automatic_change(app);
        }
    });

    tauri::Builder::default()
        // First, so a second launch exits before it builds a second tray. It
        // shows the running app's window instead.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            window::show(app)
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        // Only for the schedule's import and export dialogs, opened from
        // Rust; the window gets no dialog or file permissions.
        .plugin(tauri_plugin_dialog::init())
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
            timer_picker_requested: AtomicBool::new(false),
        })
        .setup(move |app| {
            let _ = handle.set(app.handle().clone());
            setup(app)
        })
        .on_window_event(|window, event| {
            if window.label() == window::MAIN && matches!(event, WindowEvent::Destroyed) {
                window::on_closed(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            set_active_command,
            start_timer_command,
            take_timer_picker_request,
            export_schedule,
            import_schedule,
            get_settings,
            update_settings,
            nudge_keys,
            open_link,
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

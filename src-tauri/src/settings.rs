//! Loading, saving and applying the settings file (SPEC §5.4, FR-14).

use std::path::PathBuf;

use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::GlobalShortcutExt as _;
use unidle_core::Settings;

use crate::App;

const FILE_NAME: &str = "settings.json";

/// `settings.json` in the platform's config directory, for example
/// `%APPDATA%\com.throughthewind.unidle` on Windows.
pub fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(FILE_NAME))
}

/// Missing or unreadable means defaults; never fails (FR-15).
pub fn load(app: &AppHandle) -> Settings {
    let Some(text) = path(app).and_then(|path| std::fs::read_to_string(path).ok()) else {
        return Settings::default();
    };
    let (settings, fallbacks) = Settings::from_json(&text);
    if !fallbacks.is_empty() {
        eprintln!("settings: using defaults for {}", fallbacks.join(", "));
    }
    settings
}

/// Writes to a temporary file and renames it over the old one, so a crash
/// mid-write never leaves a half-written file.
pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = path(app).ok_or("No config directory")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, settings.to_json()).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, &path).map_err(|error| error.to_string())
}

/// Applies the parts that live in the controller and adapter. Called at
/// startup and after every change.
pub fn apply_to_controller(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let state = app.state::<App>();
    let result = {
        let mut controller = state.controller();
        controller.set_nudge_config(settings.nudge_config());
        let adapter = controller.adapter_mut();
        adapter.set_nudge_key(settings.nudge_key);
        let power = adapter
            .set_power_options(settings.power_options())
            .map_err(|error| error.to_string());
        let schedule = if *controller.schedule() == settings.schedule() {
            Ok(())
        } else {
            // Entering or leaving a window turns ON or OFF at once.
            controller
                .set_schedule(settings.schedule())
                .map_err(|error| format!("The schedule could not turn activity on: {error}"))
        };
        let conditions = if *controller.conditions() == settings.conditions() {
            Ok(())
        } else {
            controller
                .set_conditions(settings.conditions())
                .map_err(|error| format!("Activity could not resume: {error}"))
        };
        power.and(schedule).and(conditions)
    };
    crate::activity_log::apply(app, settings.activity_log);
    state.ticker.wake();
    result
}

/// Registers the global shortcut, replacing any earlier one (FR-2).
pub fn apply_shortcut(app: &AppHandle, shortcut: Option<&str>) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    match shortcut {
        None => Ok(()),
        Some(shortcut) => shortcuts
            .register(shortcut)
            .map_err(|error| format!("The shortcut {shortcut} can't be used: {error}")),
    }
}

fn apply_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|error| format!("Launch at login could not be changed: {error}"))
}

/// Merges `patch` (camelCase fields, any subset) into the current settings,
/// applies what changed, saves, and returns the new settings. A value that
/// is invalid, or a change the OS refuses, is reported and not kept.
pub fn update(app: &AppHandle, patch: Value) -> Result<Settings, String> {
    let state = app.state::<App>();
    let old = state.settings().clone();

    let Value::Object(patch) = patch else {
        return Err("Settings must be an object".into());
    };
    let mut merged = match serde_json::to_value(&old) {
        Ok(Value::Object(object)) => object,
        _ => return Err("Settings could not be read".into()),
    };
    let changed: Vec<String> = patch.keys().cloned().collect();
    merged.extend(patch);
    let (new, fallbacks) = Settings::from_json(&Value::Object(merged).to_string());
    if let Some(field) = fallbacks
        .iter()
        .find(|field| changed.iter().any(|key| key == *field))
    {
        return Err(format!("Invalid value for {field}"));
    }

    if new.global_shortcut != old.global_shortcut
        && let Err(error) = apply_shortcut(app, new.global_shortcut.as_deref())
    {
        let _ = apply_shortcut(app, old.global_shortcut.as_deref());
        return Err(error);
    }
    if new.launch_at_login != old.launch_at_login {
        apply_launch_at_login(app, new.launch_at_login)?;
    }
    apply_to_controller(app, &new)?;

    *state.settings() = new.clone();
    save(app, &new)?;
    Ok(new)
}

/// Saves one field the app itself changes, such as the last state.
pub fn record(app: &AppHandle, change: impl FnOnce(&mut Settings)) {
    let state = app.state::<App>();
    let snapshot = {
        let mut settings = state.settings();
        change(&mut settings);
        settings.clone()
    };
    if let Err(error) = save(app, &snapshot) {
        eprintln!("settings: could not save: {error}");
    }
}

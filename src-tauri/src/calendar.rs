//! Calendar awareness (SPEC §18): the meetings in a local `.ics` file turn
//! the app ON. The file is read from disk only, never fetched (NFR-2), and
//! read again when it changes, so a calendar app that exports to it on a
//! schedule keeps the meetings current.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use unidle_core::Settings;
use unidle_core::calendar::{Calendar, MAX_FILE_BYTES};

use crate::{App, strings};

/// How often the file is checked for changes: one `stat`.
const CHECK_EVERY: Duration = Duration::from_secs(60);
/// How often the meetings are worked out again, so the range moves on.
const EXPAND_EVERY: Duration = Duration::from_secs(60 * 60);
/// The meetings the controller gets: from a day back, so one that started
/// before the last expansion still counts, to two weeks ahead.
const BEHIND: SignedDuration = SignedDuration::from_hours(24);
const AHEAD: SignedDuration = SignedDuration::from_hours(15 * 24);

#[derive(Default)]
struct Watch {
    path: Option<PathBuf>,
    /// The file's modification time and size when last read.
    read: Option<(SystemTime, u64)>,
    calendar: Calendar,
    expanded: Option<Instant>,
    info: Info,
}

/// What the Settings window shows under the file.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    /// Events in the file that can turn it ON; `None` until it is read.
    events: Option<usize>,
    error: Option<String>,
}

static WATCH: Mutex<Option<Watch>> = Mutex::new(None);

fn watch() -> MutexGuard<'static, Option<Watch>> {
    WATCH
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Starts the thread that keeps the meetings current. Called once at startup.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("unidle-calendar".into())
        .spawn(move || {
            loop {
                std::thread::sleep(CHECK_EVERY);
                refresh(&app, false);
            }
        });
    if let Err(error) = spawned {
        eprintln!("calendar: {error}");
    }
}

/// Follows the file in the settings, or clears the meetings when there is
/// none. Called at startup and after every settings change.
pub fn apply(app: &AppHandle, file: Option<&str>) {
    let path = file.map(PathBuf::from);
    {
        let mut watch = watch();
        let current = watch.get_or_insert_with(Watch::default);
        if current.path == path {
            return;
        }
        *current = Watch {
            path,
            ..Watch::default()
        };
    }
    refresh(app, true);
}

/// Reads the file again if it changed, and hands the controller the
/// meetings when they changed or the range moved on.
fn refresh(app: &AppHandle, force: bool) {
    let meetings = {
        let mut guard = watch();
        let Some(watch) = guard.as_mut() else { return };
        let Some(path) = watch.path.clone() else {
            watch.info = Info::default();
            drop(guard);
            set_meetings(app, Vec::new());
            return;
        };
        let changed = match stamp(&path) {
            Ok(stamp) if Some(stamp) == watch.read => false,
            Ok(stamp) => match read(&path) {
                Ok(calendar) => {
                    watch.info = Info {
                        events: Some(calendar.len()),
                        error: None,
                    };
                    watch.calendar = calendar;
                    watch.read = Some(stamp);
                    true
                }
                Err(error) => fail(watch, error),
            },
            Err(error) if watch.info.error.is_none() => fail(watch, error),
            Err(_) => false,
        };
        let due = watch
            .expanded
            .is_none_or(|expanded| expanded.elapsed() >= EXPAND_EVERY);
        if !(force || changed || due) {
            return;
        }
        watch.expanded = Some(Instant::now());
        let now = Timestamp::now();
        let tz = TimeZone::system();
        let from = now.checked_sub(BEHIND).unwrap_or(now);
        let to = now.checked_add(AHEAD).unwrap_or(now);
        watch.calendar.busy(from, to, &tz)
    };
    set_meetings(app, meetings);
}

/// An unreadable file turns nothing ON: its meetings are forgotten.
fn fail(watch: &mut Watch, error: String) -> bool {
    watch.calendar = Calendar::default();
    watch.read = None;
    watch.info = Info {
        events: None,
        error: Some(error),
    };
    true
}

fn stamp(path: &Path) -> Result<(SystemTime, u64), String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    let modified = metadata.modified().map_err(|error| error.to_string())?;
    Ok((modified, metadata.len()))
}

fn read(path: &Path) -> Result<Calendar, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(strings::CALENDAR_TOO_BIG.into());
    }
    let text = String::from_utf8_lossy(&bytes);
    if !text.contains("BEGIN:VCALENDAR") {
        return Err(strings::CALENDAR_NOT_ICS.into());
    }
    Ok(Calendar::parse(&text))
}

fn set_meetings(app: &AppHandle, meetings: Vec<(Timestamp, Timestamp)>) {
    let state = app.state::<App>();
    let (before, after) = {
        let mut controller = state.controller();
        if controller.meetings() == meetings.as_slice() {
            return;
        }
        let before = controller.status();
        if let Err(error) = controller.set_meetings(meetings) {
            eprintln!("calendar: {error}");
        }
        (before, controller.status())
    };
    state.ticker.wake();
    if before.source != after.source || before.held != after.held {
        crate::on_automatic_change(app);
    } else {
        // Same state, but the next change may be a new meeting.
        crate::tray::refresh(app, &after);
        let _ = app.emit(crate::STATUS_EVENT, crate::StatusDto::from(after));
    }
}

/// Lets the user pick the `.ics` file, and follows it. `None` when they
/// cancel. Async so the dialog doesn't block the main thread it needs.
#[tauri::command]
pub async fn choose_calendar_file(app: AppHandle) -> Result<Option<Settings>, String> {
    use tauri_plugin_dialog::DialogExt as _;
    let Some(path) = app
        .dialog()
        .file()
        .add_filter(strings::CALENDAR_FILE_FILTER, &["ics"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|error| error.to_string())?;
    let patch = serde_json::json!({ "calendarFile": path.to_string_lossy() });
    crate::settings::update(&app, patch).map(Some)
}

/// How the calendar file read, for the Settings window.
#[tauri::command]
pub fn calendar_info() -> Info {
    watch()
        .as_ref()
        .map(|watch| watch.info.clone())
        .unwrap_or_default()
}

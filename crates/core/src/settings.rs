//! The settings file (SPEC §5.4, FR-14, FR-15): human-readable JSON, read
//! leniently so that one bad value never costs the user the others.

use std::time::Duration;

use jiff::civil::Date;
use serde::Serialize;
use serde::ser::SerializeSeq as _;
use serde_json::{Map, Value};

use crate::adapter::NudgeKey;
use crate::nudge::{DEFAULT_INTERVAL, MAX_INTERVAL, MIN_INTERVAL, NudgeConfig, NudgeMethod};
use crate::platform::PowerOptions;
use crate::schedule::{Schedule, TimeWindow, Weekdays, format_time, parse_time};

/// Enough for any real week, small enough that a bad file can't make every
/// tick slow.
pub const MAX_SCHEDULE_WINDOWS: usize = 50;
pub const MAX_SKIP_DATES: usize = 400;
/// The format of schedule files, so a later format can be told apart.
const SCHEDULE_FILE_VERSION: u64 = 1;

/// The window's ON/OFF button designs (design system). The first is the
/// default.
pub const BUTTON_STYLES: [&str; 4] = ["halo", "orbit", "pulse", "eye"];

/// The window's color themes. The first, following the OS, is the default.
pub const THEMES: [&str; 3] = ["system", "light", "dark"];

/// The app icon designs: the Wake eye and the Halo power button. The first
/// is the default and the one the installers bundle.
pub const APP_ICONS: [&str; 2] = ["eye", "power"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Off means the power assertion only.
    pub nudge_enabled: bool,
    #[serde(serialize_with = "method_name")]
    pub nudge_method: NudgeMethod,
    #[serde(serialize_with = "key_name")]
    pub nudge_key: NudgeKey,
    /// Seconds, 10 to 600.
    pub nudge_interval_secs: u64,
    /// Off lets the screen turn off while the system stays awake.
    pub keep_display_on: bool,
    pub launch_at_login: bool,
    /// When off, the app always starts OFF.
    pub restore_last_state: bool,
    /// Whether simulation was ON when the app last changed state. Only read
    /// when `restore_last_state` is on.
    pub last_state_on: bool,
    /// For example `Ctrl+Alt+A`. Unset by default.
    pub global_shortcut: Option<String>,
    /// OS notification on each state change (SPEC FR-16).
    pub notify_on_change: bool,
    /// The one-time "still running in the tray" notice was shown (FR-12).
    pub tray_notice_shown: bool,
    /// One of [`BUTTON_STYLES`].
    pub button_style: String,
    /// One of [`THEMES`].
    pub theme: String,
    /// One of [`APP_ICONS`]. Sets the tray, window and Dock icons.
    pub app_icon: String,
    /// The quick timers under the button. The tray menu always has them.
    pub show_timers: bool,
    /// A left-click on the tray icon turns ON or OFF instead of opening the
    /// menu. The default follows each OS's habit: macOS menu bar items open
    /// their menu.
    pub tray_click_toggles: bool,
    /// Turn ON and OFF on the weekly schedule (ROADMAP Phase 2).
    pub schedule_enabled: bool,
    /// `{"days": ["mon", …], "start": "09:00", "end": "18:00"}` each.
    #[serde(serialize_with = "windows_json")]
    pub schedule_windows: Vec<TimeWindow>,
    /// `"2026-12-25"` each: days the schedule doesn't run.
    #[serde(serialize_with = "dates_json")]
    pub skip_dates: Vec<Date>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nudge_enabled: true,
            nudge_method: NudgeMethod::Pointer,
            nudge_key: NudgeKey::platform_default(),
            nudge_interval_secs: DEFAULT_INTERVAL.as_secs(),
            keep_display_on: true,
            launch_at_login: false,
            restore_last_state: false,
            last_state_on: false,
            global_shortcut: None,
            notify_on_change: false,
            tray_notice_shown: false,
            button_style: BUTTON_STYLES[0].to_string(),
            theme: THEMES[0].to_string(),
            app_icon: APP_ICONS[0].to_string(),
            show_timers: true,
            tray_click_toggles: !cfg!(target_os = "macos"),
            schedule_enabled: false,
            schedule_windows: Vec::new(),
            skip_dates: Vec::new(),
        }
    }
}

impl Settings {
    /// Reads settings JSON. Anything missing, of the wrong type or out of
    /// range takes its default, and the file as a whole being unreadable
    /// gives all defaults (FR-15). Returns the names of the fields that fell
    /// back, so the caller can log them.
    pub fn from_json(text: &str) -> (Self, Vec<&'static str>) {
        let mut fallbacks = Vec::new();
        let object = match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(object)) => object,
            _ => {
                fallbacks.push("(whole file)");
                Map::new()
            }
        };
        let defaults = Self::default();
        let mut read = Reader {
            object: &object,
            fallbacks: &mut fallbacks,
        };

        let settings = Self {
            nudge_enabled: read.bool("nudgeEnabled", defaults.nudge_enabled),
            nudge_method: read.parse("nudgeMethod", defaults.nudge_method, |value| {
                match value.as_str()? {
                    "pointer" => Some(NudgeMethod::Pointer),
                    "keystroke" => Some(NudgeMethod::Keystroke),
                    "both" => Some(NudgeMethod::Both),
                    _ => None,
                }
            }),
            nudge_key: read.parse("nudgeKey", defaults.nudge_key, |value| {
                NudgeKey::from_name(value.as_str()?).filter(|key| key.is_safe_here())
            }),
            nudge_interval_secs: read.parse(
                "nudgeIntervalSecs",
                defaults.nudge_interval_secs,
                |value| {
                    value.as_u64().filter(|secs| {
                        (MIN_INTERVAL.as_secs()..=MAX_INTERVAL.as_secs()).contains(secs)
                    })
                },
            ),
            keep_display_on: read.bool("keepDisplayOn", defaults.keep_display_on),
            launch_at_login: read.bool("launchAtLogin", defaults.launch_at_login),
            restore_last_state: read.bool("restoreLastState", defaults.restore_last_state),
            last_state_on: read.bool("lastStateOn", defaults.last_state_on),
            global_shortcut: read.parse("globalShortcut", None, |value| match value {
                Value::Null => Some(None),
                Value::String(text) if text.trim().is_empty() => Some(None),
                Value::String(text) => Some(Some(text.trim().to_string())),
                _ => None,
            }),
            notify_on_change: read.bool("notifyOnChange", defaults.notify_on_change),
            tray_notice_shown: read.bool("trayNoticeShown", defaults.tray_notice_shown),
            button_style: read.parse("buttonStyle", defaults.button_style.clone(), |value| {
                let style = value.as_str()?;
                BUTTON_STYLES.contains(&style).then(|| style.to_string())
            }),
            theme: read.parse("theme", defaults.theme.clone(), |value| {
                let theme = value.as_str()?;
                THEMES.contains(&theme).then(|| theme.to_string())
            }),
            app_icon: read.parse("appIcon", defaults.app_icon.clone(), |value| {
                let icon = value.as_str()?;
                APP_ICONS.contains(&icon).then(|| icon.to_string())
            }),
            show_timers: read.bool("showTimers", defaults.show_timers),
            tray_click_toggles: read.bool("trayClickToggles", defaults.tray_click_toggles),
            schedule_enabled: read.bool("scheduleEnabled", defaults.schedule_enabled),
            schedule_windows: read.parse("scheduleWindows", Vec::new(), |value| {
                let windows = value.as_array()?;
                if windows.len() > MAX_SCHEDULE_WINDOWS {
                    return None;
                }
                windows.iter().map(window_from_json).collect()
            }),
            skip_dates: read.parse("skipDates", Vec::new(), |value| {
                let dates = value.as_array()?;
                if dates.len() > MAX_SKIP_DATES {
                    return None;
                }
                let mut dates: Vec<Date> = dates
                    .iter()
                    .map(|date| date.as_str()?.parse().ok())
                    .collect::<Option<_>>()?;
                dates.sort_unstable();
                dates.dedup();
                Some(dates)
            }),
        };
        (settings, fallbacks)
    }

    pub fn to_json(&self) -> String {
        // Serializing plain fields cannot fail.
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }

    pub fn nudge_config(&self) -> NudgeConfig {
        NudgeConfig {
            enabled: self.nudge_enabled,
            method: self.nudge_method,
            interval: Duration::from_secs(self.nudge_interval_secs),
        }
        .clamped()
    }

    pub fn power_options(&self) -> PowerOptions {
        PowerOptions {
            keep_display_on: self.keep_display_on,
        }
    }

    pub fn schedule(&self) -> Schedule {
        Schedule {
            enabled: self.schedule_enabled,
            windows: self.schedule_windows.clone(),
            skip_dates: self.skip_dates.clone(),
        }
    }

    /// The schedule as a file to share or back up (ROADMAP Phase 2): the
    /// windows and days off, without the on/off switch.
    pub fn schedule_file(&self) -> String {
        let value = serde_json::json!({
            "unidleSchedule": SCHEDULE_FILE_VERSION,
            "scheduleWindows": serde_json::to_value(self).ok().and_then(|v| v.get("scheduleWindows").cloned()),
            "skipDates": self.skip_dates.iter().map(ToString::to_string).collect::<Vec<_>>(),
        });
        serde_json::to_string_pretty(&value).unwrap_or_default() + "\n"
    }

    /// Reads a file written by [`Self::schedule_file`] into a settings patch.
    /// Unlike the settings file, it is all or nothing: a file with anything
    /// wrong is refused, so an import never half-replaces a schedule.
    pub fn schedule_patch_from_file(text: &str) -> Result<Value, String> {
        const NOT_A_SCHEDULE: &str = "This isn't an Unidle schedule file";
        let Ok(Value::Object(object)) = serde_json::from_str::<Value>(text) else {
            return Err(NOT_A_SCHEDULE.into());
        };
        if object.get("unidleSchedule").and_then(Value::as_u64) != Some(SCHEDULE_FILE_VERSION) {
            return Err(NOT_A_SCHEDULE.into());
        }
        let mut patch = Map::new();
        for key in ["scheduleWindows", "skipDates"] {
            patch.insert(
                key.into(),
                object.get(key).cloned().unwrap_or(Value::Array(Vec::new())),
            );
        }
        let (_, fallbacks) = Self::from_json(&Value::Object(patch.clone()).to_string());
        match fallbacks.first() {
            None => Ok(Value::Object(patch)),
            Some(&"scheduleWindows") => Err("A time in this file is invalid".into()),
            Some(_) => Err("A day off in this file is invalid".into()),
        }
    }

    /// Whether the app should turn ON as it starts.
    pub fn start_on(&self) -> bool {
        self.restore_last_state && self.last_state_on
    }
}

struct Reader<'a> {
    object: &'a Map<String, Value>,
    fallbacks: &'a mut Vec<&'static str>,
}

impl Reader<'_> {
    fn parse<T>(
        &mut self,
        key: &'static str,
        default: T,
        parse: impl Fn(&Value) -> Option<T>,
    ) -> T {
        match self.object.get(key) {
            None => default,
            Some(value) => parse(value).unwrap_or_else(|| {
                self.fallbacks.push(key);
                default
            }),
        }
    }

    fn bool(&mut self, key: &'static str, default: bool) -> bool {
        self.parse(key, default, Value::as_bool)
    }
}

fn method_name<S: serde::Serializer>(
    method: &NudgeMethod,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(match method {
        NudgeMethod::Pointer => "pointer",
        NudgeMethod::Keystroke => "keystroke",
        NudgeMethod::Both => "both",
    })
}

fn window_from_json(value: &Value) -> Option<TimeWindow> {
    let days = value
        .get("days")?
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()?;
    let start = parse_time(value.get("start")?.as_str()?)?;
    let end = parse_time(value.get("end")?.as_str()?)?;
    TimeWindow::new(Weekdays::from_names(days)?, start, end)
}

fn windows_json<S: serde::Serializer>(
    windows: &[TimeWindow],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut seq = serializer.serialize_seq(Some(windows.len()))?;
    for window in windows {
        seq.serialize_element(&serde_json::json!({
            "days": window.days.names(),
            "start": format_time(window.start),
            "end": format_time(window.end),
        }))?;
    }
    seq.end()
}

fn dates_json<S: serde::Serializer>(dates: &[Date], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(dates.iter().map(ToString::to_string))
}

fn key_name<S: serde::Serializer>(key: &NudgeKey, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(key.name())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let settings = Settings {
            nudge_enabled: false,
            nudge_method: NudgeMethod::Both,
            nudge_key: NudgeKey::F13,
            nudge_interval_secs: 120,
            keep_display_on: false,
            launch_at_login: true,
            restore_last_state: true,
            last_state_on: true,
            global_shortcut: Some("Ctrl+Alt+A".into()),
            notify_on_change: true,
            tray_notice_shown: true,
            button_style: "eye".into(),
            theme: "dark".into(),
            app_icon: "power".into(),
            show_timers: false,
            tray_click_toggles: cfg!(target_os = "macos"),
            schedule_enabled: true,
            schedule_windows: vec![
                TimeWindow::new(Weekdays::WORKDAYS, 9 * 60, 18 * 60).unwrap(),
                TimeWindow::new(Weekdays::from_names(["sat"]).unwrap(), 22 * 60, 1440).unwrap(),
            ],
            skip_dates: vec![jiff::civil::date(2026, 12, 25)],
        };
        let (read, fallbacks) = Settings::from_json(&settings.to_json());
        assert_eq!(read, settings);
        assert!(fallbacks.is_empty());
    }

    #[test]
    fn missing_file_content_gives_defaults() {
        for text in ["", "not json", "[1, 2]", "null"] {
            let (settings, fallbacks) = Settings::from_json(text);
            assert_eq!(settings, Settings::default(), "{text:?}");
            assert_eq!(fallbacks, ["(whole file)"]);
        }
        let (settings, fallbacks) = Settings::from_json("{}");
        assert_eq!(settings, Settings::default());
        assert!(fallbacks.is_empty());
    }

    #[test]
    fn a_bad_value_falls_back_alone() {
        let (settings, fallbacks) = Settings::from_json(
            r#"{
                "nudgeEnabled": "yes",
                "nudgeMethod": "wiggle",
                "nudgeKey": "Alt+F4",
                "nudgeIntervalSecs": 5,
                "keepDisplayOn": false,
                "globalShortcut": 42,
                "buttonStyle": "sparkles",
                "theme": "sepia",
                "appIcon": "cat",
                "showTimers": "yes",
                "trayClickToggles": 1,
                "scheduleWindows": [{"days": ["mon"], "start": "09:00", "end": "25:00"}],
                "skipDates": ["2026-02-30"]
            }"#,
        );
        assert_eq!(
            settings,
            Settings {
                keep_display_on: false,
                ..Settings::default()
            }
        );
        assert_eq!(
            fallbacks,
            [
                "nudgeEnabled",
                "nudgeMethod",
                "nudgeKey",
                "nudgeIntervalSecs",
                "globalShortcut",
                "buttonStyle",
                "theme",
                "appIcon",
                "showTimers",
                "trayClickToggles",
                "scheduleWindows",
                "skipDates"
            ]
        );
    }

    #[test]
    fn the_interval_must_be_in_range() {
        for (secs, expected) in [(9, 60), (10, 10), (600, 600), (601, 60)] {
            let (settings, _) = Settings::from_json(&format!(r#"{{"nudgeIntervalSecs": {secs}}}"#));
            assert_eq!(settings.nudge_interval_secs, expected, "{secs}");
        }
    }

    #[test]
    fn a_blank_shortcut_is_unset() {
        let (settings, _) = Settings::from_json(r#"{"globalShortcut": "  "}"#);
        assert_eq!(settings.global_shortcut, None);
    }

    #[test]
    fn starts_on_only_when_restoring_an_on_state() {
        let on = |restore, last| {
            Settings {
                restore_last_state: restore,
                last_state_on: last,
                ..Settings::default()
            }
            .start_on()
        };
        assert!(!on(false, true));
        assert!(!on(true, false));
        assert!(on(true, true));
    }

    #[test]
    fn the_defaults_match_the_spec() {
        let settings = Settings::default();
        assert!(settings.nudge_enabled);
        assert_eq!(settings.nudge_method, NudgeMethod::Pointer);
        assert_eq!(settings.nudge_interval_secs, 60);
        assert!(settings.keep_display_on);
        assert!(!settings.launch_at_login);
        assert!(!settings.restore_last_state);
        assert_eq!(settings.global_shortcut, None);
        assert!(!settings.notify_on_change);
        assert_eq!(settings.button_style, "halo");
        assert_eq!(settings.theme, "system");
        assert_eq!(settings.app_icon, "eye");
        assert!(settings.show_timers);
        assert_eq!(settings.tray_click_toggles, !cfg!(target_os = "macos"));
        assert!(!settings.schedule_enabled);
        assert!(settings.schedule_windows.is_empty());
    }

    #[test]
    fn schedule_json_reads_like_people_write_it() {
        let (settings, fallbacks) = Settings::from_json(
            r#"{
                "scheduleEnabled": true,
                "scheduleWindows": [
                    {"days": ["mon", "tue", "wed", "thu", "fri"], "start": "09:00", "end": "18:00"},
                    {"days": ["sun"], "start": "22:00", "end": "06:00"}
                ],
                "skipDates": ["2026-12-25", "2026-12-24", "2026-12-25"]
            }"#,
        );
        assert!(fallbacks.is_empty());
        let schedule = settings.schedule();
        assert!(schedule.is_active());
        assert_eq!(schedule.windows[0].days, Weekdays::WORKDAYS);
        assert_eq!(
            (schedule.windows[1].start, schedule.windows[1].end),
            (1320, 360)
        );
        assert_eq!(
            schedule.skip_dates,
            [
                jiff::civil::date(2026, 12, 24),
                jiff::civil::date(2026, 12, 25)
            ],
            "sorted, without duplicates"
        );
    }

    #[test]
    fn a_schedule_file_round_trips_without_the_switch() {
        let (settings, _) = Settings::from_json(
            r#"{
                "scheduleEnabled": true,
                "scheduleWindows": [{"days": ["mon"], "start": "09:00", "end": "12:00"}],
                "skipDates": ["2026-12-25"]
            }"#,
        );
        let file = settings.schedule_file();
        assert!(!file.contains("scheduleEnabled"));
        let patch = Settings::schedule_patch_from_file(&file).unwrap();
        let (read, fallbacks) = Settings::from_json(&patch.to_string());
        assert!(fallbacks.is_empty());
        assert_eq!(read.schedule_windows, settings.schedule_windows);
        assert_eq!(read.skip_dates, settings.skip_dates);
        assert!(!read.schedule_enabled, "the switch stays the user's");
    }

    #[test]
    fn a_bad_schedule_file_is_refused_whole() {
        for text in ["", "{}", r#"{"unidleSchedule": 2}"#, "[1]"] {
            assert!(Settings::schedule_patch_from_file(text).is_err(), "{text}");
        }
        let bad_time = r#"{"unidleSchedule": 1, "scheduleWindows": [{"days": ["mon"], "start": "9", "end": "12:00"}]}"#;
        assert_eq!(
            Settings::schedule_patch_from_file(bad_time).unwrap_err(),
            "A time in this file is invalid"
        );
        let bad_date = r#"{"unidleSchedule": 1, "skipDates": ["tomorrow"]}"#;
        assert!(Settings::schedule_patch_from_file(bad_date).is_err());
        // Missing lists mean empty ones.
        let empty = Settings::schedule_patch_from_file(r#"{"unidleSchedule": 1}"#).unwrap();
        assert_eq!(empty["scheduleWindows"], Value::Array(Vec::new()));
    }

    #[test]
    fn a_window_without_days_is_invalid() {
        let (settings, fallbacks) = Settings::from_json(
            r#"{"scheduleWindows": [{"days": [], "start": "09:00", "end": "18:00"}]}"#,
        );
        assert!(settings.schedule_windows.is_empty());
        assert_eq!(fallbacks, ["scheduleWindows"]);
    }
}

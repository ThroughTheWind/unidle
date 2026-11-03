//! The settings file (SPEC §5.4, FR-14, FR-15): human-readable JSON, read
//! leniently so that one bad value never costs the user the others.

use std::time::Duration;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::adapter::NudgeKey;
use crate::nudge::{DEFAULT_INTERVAL, MAX_INTERVAL, MIN_INTERVAL, NudgeConfig, NudgeMethod};
use crate::platform::PowerOptions;

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
                "appIcon": "cat"
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
                "appIcon"
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
    }
}

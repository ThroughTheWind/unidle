use std::fmt;
use std::time::Duration;

use crate::platform::PowerSource;

/// Why the input nudge cannot run. The app still turns ON with the power
/// assertion alone and tells the user (SPEC PB-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputUnavailable {
    /// macOS: the app has not been granted the Accessibility permission.
    NeedsPermission,
    /// Linux: a Wayland session, where no portable way to inject input exists.
    Wayland,
    /// Linux: no X display to send input to.
    NoDisplay,
    /// The OS or display server lacks what the nudge needs.
    Unsupported,
}

/// What a platform can do right now. The app still turns ON when a capability
/// is missing, and tells the user what it could not do (SPEC PB-2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub power_assertion: bool,
    pub input_nudge: bool,
    /// Set whenever `input_nudge` is false.
    pub input_unavailable: Option<InputUnavailable>,
}

/// One synthetic input event with no net visible effect (SPEC FR-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NudgeKind {
    /// Move the pointer 1 px and straight back.
    Pointer,
    /// Press and release the configured [`NudgeKey`].
    Keystroke,
}

/// Keys a keystroke nudge may press: none of them types a character or has a
/// default action (SPEC FR-7). The list is closed on purpose, so no setting
/// can make the nudge press a key that does something.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NudgeKey {
    F13,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    Shift,
}

impl NudgeKey {
    pub const ALL: [NudgeKey; 8] = [
        NudgeKey::F13,
        NudgeKey::F15,
        NudgeKey::F16,
        NudgeKey::F17,
        NudgeKey::F18,
        NudgeKey::F19,
        NudgeKey::F20,
        NudgeKey::Shift,
    ];

    /// F15 on Windows and Linux, Shift on macOS (SPEC FR-7).
    pub const fn platform_default() -> Self {
        if cfg!(target_os = "macos") {
            NudgeKey::Shift
        } else {
            NudgeKey::F15
        }
    }

    /// macOS maps F14 and F15 to display brightness on many keyboards, so
    /// F15 is refused there. F14 is not offered anywhere for the same reason.
    pub const fn is_safe_here(self) -> bool {
        !(cfg!(target_os = "macos") && matches!(self, NudgeKey::F15))
    }

    /// The key itself if it is safe on this OS, otherwise the default.
    pub const fn or_safe_default(self) -> Self {
        if self.is_safe_here() {
            self
        } else {
            Self::platform_default()
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            NudgeKey::F13 => "F13",
            NudgeKey::F15 => "F15",
            NudgeKey::F16 => "F16",
            NudgeKey::F17 => "F17",
            NudgeKey::F18 => "F18",
            NudgeKey::F19 => "F19",
            NudgeKey::F20 => "F20",
            NudgeKey::Shift => "Shift",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|key| key.name().eq_ignore_ascii_case(name))
    }
}

impl Default for NudgeKey {
    fn default() -> Self {
        Self::platform_default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterError(pub String);

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AdapterError {}

/// The OS-facing interface every platform implements (SPEC §11).
pub trait PlatformAdapter {
    fn acquire(&mut self) -> Result<(), AdapterError>;
    fn release(&mut self) -> Result<(), AdapterError>;
    /// Sends one nudge. Never clicks, scrolls, types a character or changes
    /// focus (SPEC FR-9).
    fn nudge(&mut self, kind: NudgeKind) -> Result<(), AdapterError>;
    /// Time since the last input event the OS saw, synthetic ones included,
    /// or `None` when the OS cannot say.
    fn idle_time(&mut self) -> Option<Duration>;
    fn capabilities(&self) -> Capabilities;

    /// Whether the screen is locked, for the conditions (SPEC §14). `None`
    /// when the OS can't say.
    fn screen_locked(&mut self) -> Option<bool> {
        None
    }

    fn power_source(&mut self) -> Option<PowerSource> {
        None
    }

    /// Running processes, normalized with
    /// [`crate::platform::normalize_process_name`].
    fn process_names(&mut self) -> Option<Vec<String>> {
        None
    }

    /// The total time asleep since the computer started.
    fn time_asleep(&mut self) -> Option<Duration> {
        None
    }
}

/// Does nothing and reports no capabilities. Useful in tests and on an OS
/// with no adapter, so the UI can say that nothing is being simulated.
#[derive(Debug, Default)]
pub struct NoopAdapter;

impl PlatformAdapter for NoopAdapter {
    fn acquire(&mut self) -> Result<(), AdapterError> {
        Ok(())
    }

    fn release(&mut self) -> Result<(), AdapterError> {
        Ok(())
    }

    fn nudge(&mut self, _: NudgeKind) -> Result<(), AdapterError> {
        Ok(())
    }

    fn idle_time(&mut self) -> Option<Duration> {
        None
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            input_unavailable: Some(InputUnavailable::Unsupported),
            ..Capabilities::default()
        }
    }
}

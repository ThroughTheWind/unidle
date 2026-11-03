//! The only place in the app that calls OS APIs (SPEC §11). Each submodule
//! implements the same small surface for one OS, and everything above this
//! module is platform-independent.

use std::time::Duration;

use crate::adapter::{AdapterError, InputUnavailable, NudgeKey, NudgeKind};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as sys;
#[cfg(target_os = "macos")]
use macos as sys;
#[cfg(windows)]
use windows as sys;

/// The reason the OS shows next to the assertion, for example in
/// `powercfg /requests` or `pmset -g assertions`.
pub(crate) const REASON: &str = "Unidle is keeping this computer awake";

/// What the power assertion asks the OS for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerOptions {
    /// Prevent display sleep as well as idle system sleep. When false, the
    /// screen may turn off while the system stays awake (SPEC §5.4).
    pub keep_display_on: bool,
}

impl Default for PowerOptions {
    fn default() -> Self {
        Self {
            keep_display_on: true,
        }
    }
}

/// An OS power assertion, held until dropped.
///
/// It only ever prevents *idle* sleep. It never blocks a sleep the user asks
/// for, such as closing the lid or choosing Sleep (SPEC PB-4). Every
/// mechanism used here is owned by the process, so the OS drops it when the
/// process dies, including on a crash (SPEC PB-3).
pub struct PowerAssertion {
    _inner: sys::PowerAssertion,
}

impl PowerAssertion {
    pub fn acquire(options: PowerOptions) -> Result<Self, AdapterError> {
        Ok(Self {
            _inner: sys::PowerAssertion::acquire(options)?,
        })
    }
}

/// Whether this build has a power assertion for the current OS at all.
pub const POWER_ASSERTION_SUPPORTED: bool =
    cfg!(any(windows, target_os = "macos", target_os = "linux"));

/// Synthetic input for the nudge, and the OS's idle timer (SPEC FR-7, FR-8).
///
/// Every nudge leaves the pointer exactly where it was and types nothing
/// (SPEC FR-9). Nothing here reads what the user types: only the time since
/// the last input event (NFR-2).
pub struct Input {
    inner: sys::Input,
    key: NudgeKey,
}

impl Input {
    pub fn new() -> Self {
        Self {
            inner: sys::Input::new(),
            key: NudgeKey::platform_default(),
        }
    }

    pub fn key(&self) -> NudgeKey {
        self.key
    }

    /// An unsafe key for this OS is replaced by the default.
    pub fn set_key(&mut self, key: NudgeKey) {
        self.key = key.or_safe_default();
    }

    /// `Err` with the reason when nudges cannot be sent right now. Cheap
    /// enough to call on every status read: on macOS the permission can be
    /// granted while the app runs.
    pub fn availability(&self) -> Result<(), InputUnavailable> {
        self.inner.availability()
    }

    pub fn nudge(&mut self, kind: NudgeKind) -> Result<(), AdapterError> {
        self.availability()
            .map_err(|reason| AdapterError(format!("Input nudge unavailable: {reason:?}")))?;
        match kind {
            NudgeKind::Pointer => self.inner.nudge_pointer(),
            NudgeKind::Keystroke => self.inner.nudge_key(self.key),
        }
    }

    pub fn idle_time(&mut self) -> Option<Duration> {
        self.inner.idle_time()
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

/// macOS only: shows the system prompt that adds the app to the
/// Accessibility list. Does nothing elsewhere. Returns whether the
/// permission is already granted.
pub fn request_input_permission() -> bool {
    #[cfg(target_os = "macos")]
    return macos::request_accessibility();
    #[cfg(not(target_os = "macos"))]
    return true;
}

/// macOS only: opens System Settings at the Accessibility pane (SPEC PB-2).
pub fn open_input_permission_settings() -> Result<(), AdapterError> {
    #[cfg(target_os = "macos")]
    return macos::open_accessibility_settings();
    #[cfg(not(target_os = "macos"))]
    return Ok(());
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod sys {
    use std::time::Duration;

    use super::{AdapterError, InputUnavailable, PowerOptions};

    pub struct PowerAssertion;

    impl PowerAssertion {
        pub fn acquire(_: PowerOptions) -> Result<Self, AdapterError> {
            Err(AdapterError("Unsupported operating system".into()))
        }
    }

    pub struct Input;

    impl Input {
        pub fn new() -> Self {
            Self
        }
        pub fn availability(&self) -> Result<(), InputUnavailable> {
            Err(InputUnavailable::Unsupported)
        }
        pub fn nudge_pointer(&mut self) -> Result<(), AdapterError> {
            Err(AdapterError("Unsupported operating system".into()))
        }
        pub fn nudge_key(&mut self, _: super::NudgeKey) -> Result<(), AdapterError> {
            Err(AdapterError("Unsupported operating system".into()))
        }
        pub fn idle_time(&mut self) -> Option<Duration> {
            None
        }
    }
}

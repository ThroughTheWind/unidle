//! The only place in the app that calls OS APIs (SPEC §11). Each submodule
//! implements the same small surface for one OS, and everything above this
//! module is platform-independent.

use crate::adapter::AdapterError;

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

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod sys {
    use super::{AdapterError, PowerOptions};

    pub struct PowerAssertion;

    impl PowerAssertion {
        pub fn acquire(_: PowerOptions) -> Result<Self, AdapterError> {
            Err(AdapterError("Unsupported operating system".into()))
        }
    }
}

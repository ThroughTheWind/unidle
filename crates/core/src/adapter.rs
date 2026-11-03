use std::fmt;
use std::time::Duration;

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
    /// Press and release a key that types nothing and has no default action.
    Keystroke,
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

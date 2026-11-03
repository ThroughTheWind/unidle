use std::time::Duration;

use crate::adapter::{AdapterError, Capabilities, NudgeKey, NudgeKind, PlatformAdapter};
use crate::platform::{Input, POWER_ASSERTION_SUPPORTED, PowerAssertion, PowerOptions};

/// The real adapter: holds an OS power assertion while ON, and sends input
/// nudges where the OS allows it.
pub struct SystemAdapter {
    options: PowerOptions,
    assertion: Option<PowerAssertion>,
    input: Input,
}

impl SystemAdapter {
    pub fn new(options: PowerOptions) -> Self {
        Self {
            options,
            assertion: None,
            input: Input::new(),
        }
    }

    pub fn power_options(&self) -> PowerOptions {
        self.options
    }

    /// Changes what the assertion asks for. While held, the assertion is
    /// replaced at once, taking the new one before dropping the old so the
    /// machine is never briefly unprotected.
    pub fn set_power_options(&mut self, options: PowerOptions) -> Result<(), AdapterError> {
        if options == self.options {
            return Ok(());
        }
        if self.assertion.is_some() {
            self.assertion = Some(PowerAssertion::acquire(options)?);
        }
        self.options = options;
        Ok(())
    }

    pub fn nudge_key(&self) -> NudgeKey {
        self.input.key()
    }

    /// Takes effect from the next keystroke nudge. A key that is unsafe on
    /// this OS is replaced by the default.
    pub fn set_nudge_key(&mut self, key: NudgeKey) {
        self.input.set_key(key);
    }

    pub fn is_holding(&self) -> bool {
        self.assertion.is_some()
    }
}

impl Default for SystemAdapter {
    fn default() -> Self {
        Self::new(PowerOptions::default())
    }
}

impl PlatformAdapter for SystemAdapter {
    fn acquire(&mut self) -> Result<(), AdapterError> {
        if self.assertion.is_none() {
            self.assertion = Some(PowerAssertion::acquire(self.options)?);
        }
        Ok(())
    }

    fn release(&mut self) -> Result<(), AdapterError> {
        self.assertion = None;
        Ok(())
    }

    fn nudge(&mut self, kind: NudgeKind) -> Result<(), AdapterError> {
        self.input.nudge(kind)
    }

    fn idle_time(&mut self) -> Option<Duration> {
        self.input.idle_time()
    }

    fn capabilities(&self) -> Capabilities {
        let input = self.input.availability();
        Capabilities {
            power_assertion: POWER_ASSERTION_SUPPORTED,
            input_nudge: input.is_ok(),
            input_unavailable: input.err(),
        }
    }
}

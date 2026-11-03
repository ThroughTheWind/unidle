use crate::adapter::{AdapterError, Capabilities, PlatformAdapter};
use crate::platform::{POWER_ASSERTION_SUPPORTED, PowerAssertion, PowerOptions};

/// The real adapter: holds an OS power assertion while ON.
pub struct SystemAdapter {
    options: PowerOptions,
    assertion: Option<PowerAssertion>,
}

impl SystemAdapter {
    pub fn new(options: PowerOptions) -> Self {
        Self {
            options,
            assertion: None,
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

    fn nudge(&mut self) -> Result<(), AdapterError> {
        Ok(())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            power_assertion: POWER_ASSERTION_SUPPORTED,
            input_nudge: false,
        }
    }
}

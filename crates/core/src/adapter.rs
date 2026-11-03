use std::fmt;

/// What a platform can do right now. The app still turns ON when a capability
/// is missing, and tells the user what it could not do (SPEC PB-2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub power_assertion: bool,
    pub input_nudge: bool,
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
    fn nudge(&mut self) -> Result<(), AdapterError>;
    fn capabilities(&self) -> Capabilities;
}

/// Does nothing and reports no capabilities. Stands in until the real
/// platform adapters land, so the UI can say that nothing is being simulated.
#[derive(Debug, Default)]
pub struct NoopAdapter;

impl PlatformAdapter for NoopAdapter {
    fn acquire(&mut self) -> Result<(), AdapterError> {
        Ok(())
    }

    fn release(&mut self) -> Result<(), AdapterError> {
        Ok(())
    }

    fn nudge(&mut self) -> Result<(), AdapterError> {
        Ok(())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
}

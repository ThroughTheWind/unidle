//! Simulation core: one controller owns the state machine, and platform
//! adapters own the OS calls. The UI talks to the controller only (SPEC §11).

mod adapter;
mod controller;

pub use adapter::{AdapterError, Capabilities, NoopAdapter, PlatformAdapter};
pub use controller::{Controller, Source, Status};

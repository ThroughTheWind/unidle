//! Simulation core: one controller owns the state machine, and platform
//! adapters own the OS calls. The UI talks to the controller only (SPEC §11).

mod adapter;
mod controller;
mod nudge;
mod platform;
mod settings;
mod system;
mod ticker;

pub use adapter::{
    AdapterError, Capabilities, InputUnavailable, NoopAdapter, NudgeKey, NudgeKind, PlatformAdapter,
};
pub use controller::{Controller, Event, EventKind, Source, Status};
pub use nudge::{DEFAULT_INTERVAL, MAX_INTERVAL, MIN_INTERVAL, NudgeConfig, NudgeMethod};
pub use platform::{
    Input, PowerAssertion, PowerOptions, open_input_permission_settings, request_input_permission,
};
pub use settings::{APP_ICONS, BUTTON_STYLES, Settings};
pub use system::SystemAdapter;
pub use ticker::Ticker;

//! Simulation core: one controller owns the state machine, and platform
//! adapters own the OS calls. The UI talks to the controller only (SPEC §11).

pub mod activity;
mod adapter;
mod automation;
mod conditions;
mod controller;
mod nudge;
mod platform;
pub mod remote;
mod schedule;
mod settings;
mod system;
mod ticker;

pub use adapter::{
    AdapterError, Capabilities, InputUnavailable, NoopAdapter, NudgeKey, NudgeKind, PlatformAdapter,
};
pub use automation::NextChange;
pub use conditions::{Conditions, Hold, Reading};
pub use controller::{
    Change, ChangeSink, Controller, Event, EventKind, SessionSink, Source, Status,
};
pub use nudge::{DEFAULT_INTERVAL, MAX_INTERVAL, MIN_INTERVAL, NudgeConfig, NudgeMethod};
pub use platform::{
    Environment, Input, ManagedSignal, Policy, PowerAssertion, PowerOptions, PowerSource,
    managed_signal, normalize_process_name, open_input_permission_settings, read_policy,
    request_input_permission,
};
pub use schedule::{
    MINUTES_PER_DAY, Schedule, TimeWindow, Weekdays, format_time, next_local_time, parse_time,
};
pub use settings::{APP_ICONS, BUTTON_STYLES, Settings, THEMES};
pub use system::SystemAdapter;
pub use ticker::Ticker;

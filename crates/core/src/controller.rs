use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::adapter::{AdapterError, Capabilities, NudgeKind, PlatformAdapter};
use crate::nudge::{Due, NudgeConfig, Planner};

/// Who turned simulation on. Timers and schedules join this in Phase 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Manual,
}

#[derive(Clone, Copy, Debug)]
enum State {
    Off,
    On { source: Source, since: Instant },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub source: Option<Source>,
    pub elapsed: Duration,
    pub capabilities: Capabilities,
    /// Whether nudges are being sent while ON: enabled and possible.
    pub nudging: bool,
}

impl Status {
    pub fn is_on(&self) -> bool {
        self.source.is_some()
    }
}

/// Something the controller did. Kept in memory only, never written to disk,
/// and never records what the user typed (SPEC §11, NFR-2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    TurnedOn(Source),
    TurnedOff,
    Nudged(NudgeKind),
    SkippedUserActive,
    NudgeFailed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub at: Instant,
    pub kind: EventKind,
}

/// The log keeps the latest events only, so a week ON uses the same memory as
/// a minute (SPEC NFR-3).
const EVENT_LOG_CAPACITY: usize = 64;
/// How long `tick` asks to wait when nothing is scheduled. State changes wake
/// the ticker early, so this only bounds a missed wake-up.
const IDLE_WAIT: Duration = Duration::from_secs(60);

/// Owns the `OFF -> ON(source) -> OFF` state machine, its adapter, and the
/// nudge schedule.
pub struct Controller<A: PlatformAdapter> {
    adapter: A,
    state: State,
    planner: Planner,
    events: VecDeque<Event>,
}

impl<A: PlatformAdapter> Controller<A> {
    /// Starts OFF, whatever the adapter can do.
    pub fn new(adapter: A) -> Self {
        Self::with_nudge_config(adapter, NudgeConfig::default())
    }

    pub fn with_nudge_config(adapter: A, config: NudgeConfig) -> Self {
        Self {
            adapter,
            state: State::Off,
            planner: Planner::new(config),
            events: VecDeque::with_capacity(EVENT_LOG_CAPACITY),
        }
    }

    pub fn turn_on(&mut self, source: Source) -> Result<(), AdapterError> {
        if matches!(self.state, State::Off) {
            self.adapter.acquire()?;
            let now = Instant::now();
            self.state = State::On { source, since: now };
            self.planner.start(now);
            self.log(now, EventKind::TurnedOn(source));
        }
        Ok(())
    }

    pub fn turn_off(&mut self) -> Result<(), AdapterError> {
        if matches!(self.state, State::On { .. }) {
            // OFF wins even if the release fails: the OS drops the assertion
            // when the process exits, and the UI must not claim to be ON.
            self.state = State::Off;
            self.planner.stop();
            self.log(Instant::now(), EventKind::TurnedOff);
            self.adapter.release()?;
        }
        Ok(())
    }

    pub fn status(&self) -> Status {
        let (source, elapsed) = match self.state {
            State::Off => (None, Duration::ZERO),
            State::On { source, since } => (Some(source), since.elapsed()),
        };
        let capabilities = self.adapter.capabilities();
        Status {
            source,
            elapsed,
            capabilities,
            nudging: source.is_some() && self.nudges_possible(capabilities),
        }
    }

    pub fn nudge_config(&self) -> NudgeConfig {
        self.planner.config()
    }

    /// Takes effect at once. While ON, the current interval restarts.
    pub fn set_nudge_config(&mut self, config: NudgeConfig) {
        self.planner.set_config(config, Instant::now());
    }

    /// Sends the nudge if one is due, and returns how long to wait before
    /// calling again. Driven by [`crate::Ticker`].
    pub fn tick(&mut self, now: Instant) -> Duration {
        if !matches!(self.state, State::On { .. })
            || !self.nudges_possible(self.adapter.capabilities())
        {
            return IDLE_WAIT;
        }
        let Some(next_at) = self.planner.next_at() else {
            return IDLE_WAIT;
        };
        if now < next_at {
            return next_at - now;
        }

        let idle = self.adapter.idle_time();
        let kind = match self.planner.due(now, idle) {
            Due::SkipUserActive => EventKind::SkippedUserActive,
            Due::Nudge(kind) => match self.adapter.nudge(kind) {
                Ok(()) => EventKind::Nudged(kind),
                Err(error) => EventKind::NudgeFailed(error.0),
            },
        };
        self.log(now, kind);
        self.planner
            .next_at()
            .map_or(IDLE_WAIT, |next| next.saturating_duration_since(now))
    }

    /// Oldest first.
    pub fn events(&self) -> impl Iterator<Item = &Event> {
        self.events.iter()
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn adapter_mut(&mut self) -> &mut A {
        &mut self.adapter
    }

    fn nudges_possible(&self, capabilities: Capabilities) -> bool {
        self.planner.config().enabled && capabilities.input_nudge
    }

    fn log(&mut self, at: Instant, kind: EventKind) {
        if self.events.len() == EVENT_LOG_CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(Event { at, kind });
    }
}

impl<A: PlatformAdapter> Drop for Controller<A> {
    fn drop(&mut self) {
        let _ = self.turn_off();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::nudge::NudgeMethod;

    #[derive(Default)]
    struct Calls {
        acquire: u32,
        release: u32,
        nudges: Vec<NudgeKind>,
    }

    struct FakeAdapter {
        calls: Rc<RefCell<Calls>>,
        fail_acquire: bool,
        fail_release: bool,
        input_nudge: bool,
        idle: Option<Duration>,
    }

    impl Default for FakeAdapter {
        fn default() -> Self {
            Self {
                calls: Rc::default(),
                fail_acquire: false,
                fail_release: false,
                input_nudge: true,
                idle: None,
            }
        }
    }

    impl PlatformAdapter for FakeAdapter {
        fn acquire(&mut self) -> Result<(), AdapterError> {
            self.calls.borrow_mut().acquire += 1;
            if self.fail_acquire {
                return Err(AdapterError("acquire failed".into()));
            }
            Ok(())
        }

        fn release(&mut self) -> Result<(), AdapterError> {
            self.calls.borrow_mut().release += 1;
            if self.fail_release {
                return Err(AdapterError("release failed".into()));
            }
            Ok(())
        }

        fn nudge(&mut self, kind: NudgeKind) -> Result<(), AdapterError> {
            self.calls.borrow_mut().nudges.push(kind);
            Ok(())
        }

        fn idle_time(&mut self) -> Option<Duration> {
            self.idle
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities {
                power_assertion: true,
                input_nudge: self.input_nudge,
                input_unavailable: None,
            }
        }
    }

    #[test]
    fn starts_off_without_touching_the_os() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let controller = Controller::new(adapter);

        assert!(!controller.status().is_on());
        assert_eq!(calls.borrow().acquire, 0);
    }

    #[test]
    fn repeated_toggles_acquire_and_release_once() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let mut controller = Controller::new(adapter);

        controller.turn_on(Source::Manual).unwrap();
        controller.turn_on(Source::Manual).unwrap();
        assert_eq!(controller.status().source, Some(Source::Manual));
        assert_eq!(calls.borrow().acquire, 1);

        controller.turn_off().unwrap();
        controller.turn_off().unwrap();
        assert!(!controller.status().is_on());
        assert_eq!(controller.status().elapsed, Duration::ZERO);
        assert_eq!(calls.borrow().release, 1);
    }

    #[test]
    fn failed_acquire_stays_off() {
        let adapter = FakeAdapter {
            fail_acquire: true,
            ..Default::default()
        };
        let mut controller = Controller::new(adapter);

        assert!(controller.turn_on(Source::Manual).is_err());
        assert!(!controller.status().is_on());
    }

    #[test]
    fn failed_release_still_reports_off() {
        let adapter = FakeAdapter {
            fail_release: true,
            ..Default::default()
        };
        let mut controller = Controller::new(adapter);
        controller.turn_on(Source::Manual).unwrap();

        assert!(controller.turn_off().is_err());
        assert!(!controller.status().is_on());
    }

    #[test]
    fn dropping_while_on_releases() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let mut controller = Controller::new(adapter);
        controller.turn_on(Source::Manual).unwrap();

        drop(controller);

        assert_eq!(calls.borrow().release, 1);
    }

    #[test]
    fn nudges_only_when_due_and_on() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let mut controller = Controller::new(adapter);

        // OFF: nothing, whenever.
        controller.tick(Instant::now() + Duration::from_secs(3600));
        assert!(calls.borrow().nudges.is_empty());

        controller.turn_on(Source::Manual).unwrap();
        let start = Instant::now();
        let wait = controller.tick(start);
        assert!(wait >= Duration::from_secs(47) && wait <= Duration::from_secs(72));
        assert!(calls.borrow().nudges.is_empty());

        controller.tick(start + Duration::from_secs(73));
        assert_eq!(calls.borrow().nudges, [NudgeKind::Pointer]);
        assert!(controller.status().nudging);

        controller.turn_off().unwrap();
        controller.tick(start + Duration::from_secs(3600));
        assert_eq!(calls.borrow().nudges.len(), 1);
    }

    #[test]
    fn user_activity_skips_and_is_logged() {
        let adapter = FakeAdapter {
            idle: Some(Duration::from_secs(2)),
            ..Default::default()
        };
        let calls = adapter.calls.clone();
        let mut controller = Controller::new(adapter);
        controller.turn_on(Source::Manual).unwrap();

        controller.tick(Instant::now() + Duration::from_secs(73));

        assert!(calls.borrow().nudges.is_empty());
        let kinds: Vec<_> = controller.events().map(|event| &event.kind).collect();
        assert_eq!(
            kinds,
            [
                &EventKind::TurnedOn(Source::Manual),
                &EventKind::SkippedUserActive
            ]
        );
    }

    #[test]
    fn no_input_capability_means_power_assertion_only() {
        let adapter = FakeAdapter {
            input_nudge: false,
            ..Default::default()
        };
        let calls = adapter.calls.clone();
        let mut controller = Controller::new(adapter);

        controller.turn_on(Source::Manual).unwrap();
        controller.tick(Instant::now() + Duration::from_secs(600));

        assert!(controller.status().is_on());
        assert!(!controller.status().nudging);
        assert!(calls.borrow().nudges.is_empty());
    }

    #[test]
    fn disabled_nudges_never_fire() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let mut controller = Controller::with_nudge_config(
            adapter,
            NudgeConfig {
                enabled: false,
                method: NudgeMethod::Both,
                ..NudgeConfig::default()
            },
        );

        controller.turn_on(Source::Manual).unwrap();
        controller.tick(Instant::now() + Duration::from_secs(600));

        assert!(calls.borrow().nudges.is_empty());
    }

    #[test]
    fn event_log_is_bounded() {
        let mut controller = Controller::new(FakeAdapter::default());
        for _ in 0..500 {
            controller.turn_on(Source::Manual).unwrap();
            controller.turn_off().unwrap();
        }
        assert_eq!(controller.events().count(), EVENT_LOG_CAPACITY);
    }
}

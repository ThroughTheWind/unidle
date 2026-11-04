use std::collections::VecDeque;
use std::time::{Duration, Instant};

use jiff::{Timestamp, Zoned};

use crate::adapter::{AdapterError, Capabilities, NudgeKind, PlatformAdapter};
use crate::automation::{Automation, NextChange};
use crate::nudge::{Due, NudgeConfig, Planner};
use crate::schedule::Schedule;

/// Who turned simulation on (SPEC §10, ROADMAP Phase 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Manual,
    /// A quick timer: ON for a while, or until a time.
    Timer,
    /// A weekly schedule window.
    Schedule,
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
    /// The next time the app turns itself ON or OFF, if it will.
    pub next_change: Option<NextChange>,
    /// When a manual OFF paused a schedule window, the time the window ends.
    pub paused_until: Option<Timestamp>,
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
    /// Still ON, for another reason: a timer ended inside a schedule window.
    SourceChanged(Source),
    Nudged(NudgeKind),
    SkippedUserActive,
    NudgeFailed(String),
    /// A timer or the schedule wanted ON and the OS refused. Retried on the
    /// next tick.
    AutomationFailed(String),
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
/// The longest wait while a timer or schedule is running. The ticker's clock
/// can stop while the computer sleeps, so this bounds how late a change is
/// noticed after waking up; the wall clock decides what the change is.
const WATCH_WAIT: Duration = Duration::from_secs(15);

/// Owns the `OFF -> ON(source) -> OFF` state machine, its adapter, the nudge
/// schedule, and who decides between manual, timer and schedule.
pub struct Controller<A: PlatformAdapter> {
    adapter: A,
    state: State,
    planner: Planner,
    automation: Automation,
    /// Set when a tick changed the state, until [`Self::take_change`].
    changed: bool,
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
            automation: Automation::default(),
            changed: false,
            events: VecDeque::with_capacity(EVENT_LOG_CAPACITY),
        }
    }

    /// The user turned it ON.
    pub fn turn_on(&mut self) -> Result<(), AdapterError> {
        self.set_manual_at(true, &Zoned::now())
    }

    /// The user turned it OFF. Also cancels a timer, and pauses a schedule
    /// window until it ends.
    pub fn turn_off(&mut self) -> Result<(), AdapterError> {
        self.set_manual_at(false, &Zoned::now())
    }

    pub fn set_manual_at(&mut self, on: bool, now: &Zoned) -> Result<(), AdapterError> {
        let previous = self.automation.clone();
        self.automation
            .set_manual(on, now.timestamp(), now.time_zone());
        let result = self.sync(now);
        if result.is_err() && on {
            // A failed ON leaves things as they were, not a pending ON.
            self.automation = previous;
        }
        result
    }

    /// ON until `until`, then back to the schedule (or OFF).
    pub fn start_timer(&mut self, until: Timestamp) -> Result<(), AdapterError> {
        self.start_timer_at(until, &Zoned::now())
    }

    pub fn start_timer_at(&mut self, until: Timestamp, now: &Zoned) -> Result<(), AdapterError> {
        if until <= now.timestamp() {
            return Err(AdapterError("The timer must end in the future".into()));
        }
        let previous = self.automation.clone();
        self.automation.start_timer(until);
        let result = self.sync(now);
        if result.is_err() {
            self.automation = previous;
        }
        result
    }

    pub fn schedule(&self) -> &Schedule {
        self.automation.schedule()
    }

    /// Takes effect at once: entering or leaving a window turns ON or OFF.
    pub fn set_schedule(&mut self, schedule: Schedule) -> Result<(), AdapterError> {
        self.set_schedule_at(schedule, &Zoned::now())
    }

    pub fn set_schedule_at(&mut self, schedule: Schedule, now: &Zoned) -> Result<(), AdapterError> {
        self.automation
            .set_schedule(schedule, now.timestamp(), now.time_zone());
        self.sync(now)
    }

    /// Brings the state in line with what the user, a timer or the schedule
    /// wants now.
    fn sync(&mut self, now: &Zoned) -> Result<(), AdapterError> {
        let at = now.timestamp();
        self.automation.expire(at);
        let wanted = self.automation.decide(at, now.time_zone()).source;
        match (self.state, wanted) {
            (State::Off, None) => {}
            (State::Off, Some(source)) => {
                self.adapter.acquire()?;
                let started = Instant::now();
                self.state = State::On {
                    source,
                    since: started,
                };
                self.planner.start(started);
                self.log(started, EventKind::TurnedOn(source));
            }
            (State::On { source, since }, Some(wanted)) => {
                if source != wanted {
                    self.state = State::On {
                        source: wanted,
                        since,
                    };
                    self.log(Instant::now(), EventKind::SourceChanged(wanted));
                }
            }
            (State::On { .. }, None) => {
                // OFF wins even if the release fails: the OS drops the
                // assertion when the process exits, and the UI must not
                // claim to be ON.
                self.state = State::Off;
                self.planner.stop();
                self.log(Instant::now(), EventKind::TurnedOff);
                self.adapter.release()?;
            }
        }
        Ok(())
    }

    pub fn status(&self) -> Status {
        self.status_at(&Zoned::now())
    }

    pub fn status_at(&self, now: &Zoned) -> Status {
        let (source, elapsed) = match self.state {
            State::Off => (None, Duration::ZERO),
            State::On { source, since } => (Some(source), since.elapsed()),
        };
        let capabilities = self.adapter.capabilities();
        let (at, tz) = (now.timestamp(), now.time_zone());
        Status {
            source,
            elapsed,
            capabilities,
            nudging: source.is_some() && self.nudges_possible(capabilities),
            next_change: self.automation.next_change(at, tz),
            paused_until: self.automation.paused_until(at, tz),
        }
    }

    pub fn nudge_config(&self) -> NudgeConfig {
        self.planner.config()
    }

    /// Takes effect at once. While ON, the current interval restarts.
    pub fn set_nudge_config(&mut self, config: NudgeConfig) {
        self.planner.set_config(config, Instant::now());
    }

    /// Applies the timer and schedule, sends the nudge if one is due, and
    /// returns how long to wait before calling again. Driven by
    /// [`crate::Ticker`].
    pub fn tick(&mut self, now: Instant) -> Duration {
        self.tick_at(now, &Zoned::now())
    }

    pub fn tick_at(&mut self, now: Instant, wall: &Zoned) -> Duration {
        let before = self.state_key();
        if let Err(error) = self.sync(wall) {
            let kind = EventKind::AutomationFailed(error.0);
            // One line per failure streak, not one per tick.
            if self.events.back().map(|event| &event.kind) != Some(&kind) {
                self.log(now, kind);
            }
        }
        if self.state_key() != before {
            self.changed = true;
        }
        let nudge_wait = self.nudge_wait(now);
        self.watch_wait(wall)
            .map_or(nudge_wait, |watch_wait| nudge_wait.min(watch_wait))
    }

    /// Whether a tick changed the state since the last call, so the UI can
    /// catch up with a timer or schedule.
    pub fn take_change(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    fn state_key(&self) -> Option<Source> {
        match self.state {
            State::Off => None,
            State::On { source, .. } => Some(source),
        }
    }

    /// How long until a timer or the schedule needs a look, if one runs.
    fn watch_wait(&self, wall: &Zoned) -> Option<Duration> {
        if !self.automation.is_watching() {
            return None;
        }
        let until_change = self
            .automation
            .next_change(wall.timestamp(), wall.time_zone())
            .and_then(|change| Duration::try_from(change.at.duration_since(wall.timestamp())).ok())
            // Lands just after the change, never just before it.
            .map(|wait| wait + Duration::from_millis(5));
        Some(until_change.map_or(WATCH_WAIT, |wait| wait.min(WATCH_WAIT)))
    }

    fn nudge_wait(&mut self, now: Instant) -> Duration {
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
        // Straight to OFF, whatever a timer or schedule wants.
        if matches!(self.state, State::On { .. }) {
            self.state = State::Off;
            let _ = self.adapter.release();
        }
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

        controller.turn_on().unwrap();
        controller.turn_on().unwrap();
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

        assert!(controller.turn_on().is_err());
        assert!(!controller.status().is_on());
    }

    #[test]
    fn failed_release_still_reports_off() {
        let adapter = FakeAdapter {
            fail_release: true,
            ..Default::default()
        };
        let mut controller = Controller::new(adapter);
        controller.turn_on().unwrap();

        assert!(controller.turn_off().is_err());
        assert!(!controller.status().is_on());
    }

    #[test]
    fn dropping_while_on_releases() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let mut controller = Controller::new(adapter);
        controller.turn_on().unwrap();

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

        controller.turn_on().unwrap();
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
        controller.turn_on().unwrap();

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

        controller.turn_on().unwrap();
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

        controller.turn_on().unwrap();
        controller.tick(Instant::now() + Duration::from_secs(600));

        assert!(calls.borrow().nudges.is_empty());
    }

    #[test]
    fn event_log_is_bounded() {
        let mut controller = Controller::new(FakeAdapter::default());
        for _ in 0..500 {
            controller.turn_on().unwrap();
            controller.turn_off().unwrap();
        }
        assert_eq!(controller.events().count(), EVENT_LOG_CAPACITY);
    }

    mod automation {
        use jiff::SignedDuration;

        use super::*;
        use crate::schedule::tests::{at, office_hours, paris};

        fn wall(time: &str) -> Zoned {
            at(time).to_zoned(paris())
        }

        #[test]
        fn a_schedule_turns_on_and_off_on_tick() {
            let adapter = FakeAdapter::default();
            let calls = adapter.calls.clone();
            let mut controller = Controller::new(adapter);
            controller
                .set_schedule_at(office_hours(), &wall("2026-11-02T08:00"))
                .unwrap();
            assert!(!controller.status().is_on());

            // Wakes up right after the window opens.
            let wait = controller.tick_at(Instant::now(), &wall("2026-11-02T08:59:58"));
            assert!(wait <= Duration::from_secs(3), "{wait:?}");
            assert!(!controller.take_change());

            controller.tick_at(Instant::now(), &wall("2026-11-02T09:00"));
            assert!(controller.take_change());
            assert!(!controller.take_change(), "taken once");
            let status = controller.status_at(&wall("2026-11-02T09:00"));
            assert_eq!(status.source, Some(Source::Schedule));
            assert_eq!(
                status.next_change,
                Some(NextChange {
                    at: at("2026-11-02T18:00"),
                    on: false
                })
            );

            controller.tick_at(Instant::now(), &wall("2026-11-02T18:00"));
            assert!(controller.take_change());
            assert!(!controller.status().is_on());
            assert_eq!(calls.borrow().acquire, 1);
            assert_eq!(calls.borrow().release, 1);
        }

        #[test]
        fn entering_a_schedule_inside_its_window_turns_on_at_once() {
            let mut controller = Controller::new(FakeAdapter::default());
            controller
                .set_schedule_at(office_hours(), &wall("2026-11-02T10:00"))
                .unwrap();
            assert_eq!(controller.status().source, Some(Source::Schedule));
        }

        #[test]
        fn manual_off_pauses_and_reports_until_when() {
            let mut controller = Controller::new(FakeAdapter::default());
            let now = wall("2026-11-02T10:00");
            controller.set_schedule_at(office_hours(), &now).unwrap();
            controller.set_manual_at(false, &now).unwrap();

            let status = controller.status_at(&now);
            assert!(!status.is_on());
            assert_eq!(status.paused_until, Some(at("2026-11-02T18:00")));
            controller.tick_at(Instant::now(), &wall("2026-11-02T12:00"));
            assert!(!controller.take_change(), "stays paused");
        }

        #[test]
        fn a_timer_turns_off_when_it_ends() {
            let adapter = FakeAdapter::default();
            let calls = adapter.calls.clone();
            let mut controller = Controller::new(adapter);
            let now = wall("2026-11-02T20:00");
            controller
                .start_timer_at(now.timestamp() + SignedDuration::from_mins(30), &now)
                .unwrap();
            let status = controller.status_at(&now);
            assert_eq!(status.source, Some(Source::Timer));
            assert_eq!(
                status.next_change,
                Some(NextChange {
                    at: at("2026-11-02T20:30"),
                    on: false
                })
            );

            // A sleep through the end: the first tick after waking turns OFF.
            controller.tick_at(Instant::now(), &wall("2026-11-03T07:00"));
            assert!(controller.take_change());
            assert!(!controller.status().is_on());
            assert_eq!(calls.borrow().release, 1);
        }

        #[test]
        fn a_timer_in_the_past_is_refused() {
            let mut controller = Controller::new(FakeAdapter::default());
            let now = wall("2026-11-02T20:00");
            assert!(
                controller
                    .start_timer_at(at("2026-11-02T19:00"), &now)
                    .is_err()
            );
            assert!(!controller.status().is_on());
        }

        #[test]
        fn a_failed_timer_start_leaves_nothing_pending() {
            let adapter = FakeAdapter {
                fail_acquire: true,
                ..Default::default()
            };
            let mut controller = Controller::new(adapter);
            let now = wall("2026-11-02T20:00");
            assert!(
                controller
                    .start_timer_at(at("2026-11-02T21:00"), &now)
                    .is_err()
            );
            assert_eq!(controller.status_at(&now).next_change, None);
        }

        #[test]
        fn a_refused_scheduled_on_is_logged_once_and_retried() {
            let adapter = FakeAdapter {
                fail_acquire: true,
                ..Default::default()
            };
            let calls = adapter.calls.clone();
            let mut controller = Controller::new(adapter);
            let _ = controller.set_schedule_at(office_hours(), &wall("2026-11-02T10:00"));
            for _ in 0..3 {
                controller.tick_at(Instant::now(), &wall("2026-11-02T10:01"));
            }
            assert_eq!(calls.borrow().acquire, 4);
            let failures = controller
                .events()
                .filter(|event| matches!(event.kind, EventKind::AutomationFailed(_)))
                .count();
            assert_eq!(failures, 1);
        }

        #[test]
        fn a_timer_ending_inside_a_window_hands_over_without_a_gap() {
            let adapter = FakeAdapter::default();
            let calls = adapter.calls.clone();
            let mut controller = Controller::new(adapter);
            let now = wall("2026-11-02T08:30");
            controller.set_schedule_at(office_hours(), &now).unwrap();
            controller
                .start_timer_at(at("2026-11-02T09:30"), &now)
                .unwrap();
            controller.tick_at(Instant::now(), &wall("2026-11-02T09:30"));
            assert!(controller.take_change());
            assert_eq!(controller.status().source, Some(Source::Schedule));
            assert_eq!(calls.borrow().acquire, 1);
            assert_eq!(calls.borrow().release, 0);
            assert!(
                controller
                    .events()
                    .any(|event| event.kind == EventKind::SourceChanged(Source::Schedule))
            );
        }

        #[test]
        fn dropping_releases_whatever_the_schedule_wants() {
            let adapter = FakeAdapter::default();
            let calls = adapter.calls.clone();
            let mut controller = Controller::new(adapter);
            controller
                .set_schedule_at(office_hours(), &wall("2026-11-02T10:00"))
                .unwrap();
            drop(controller);
            assert_eq!(calls.borrow().release, 1);
        }
    }
}

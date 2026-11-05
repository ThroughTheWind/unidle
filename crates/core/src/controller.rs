use std::collections::VecDeque;
use std::time::{Duration, Instant};

use jiff::{Timestamp, Zoned};

use crate::activity::Session;
use crate::adapter::{AdapterError, Capabilities, NudgeKind, PlatformAdapter};
use crate::automation::{Automation, NextChange};
use crate::conditions::{Conditions, Hold, Reading};
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
    On {
        source: Source,
        since: Instant,
    },
    /// Wanted ON, but a condition holds it OFF for now (SPEC §14).
    Held {
        hold: Hold,
    },
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
    /// This ON was asked for without nudges, such as by `unidle on` (SPEC
    /// §13): the power assertion only, whatever the settings say.
    pub power_only: bool,
    /// Wanted ON, but held OFF by a condition. `source` is `None` meanwhile.
    pub held: Option<Hold>,
}

impl Status {
    pub fn is_on(&self) -> bool {
        self.source.is_some()
    }

    /// ON, or wanted ON and held by a condition: what the toggle shows.
    pub fn is_wanted(&self) -> bool {
        self.is_on() || self.held.is_some()
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
    /// A condition paused it (SPEC §14).
    Held(Hold),
    /// No nudge while the screen is locked.
    SkippedLocked,
    /// The computer slept while ON, so it turned OFF.
    OffAfterSleep,
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
/// How often the conditions are read while ON or held.
const CONDITION_WAIT: Duration = Duration::from_secs(5);
/// More time asleep than this between two ticks means the computer slept.
const SLEPT: Duration = Duration::from_secs(20);

/// Owns the `OFF -> ON(source) -> OFF` state machine, its adapter, the nudge
/// schedule, and who decides between manual, timer and schedule.
pub struct Controller<A: PlatformAdapter> {
    adapter: A,
    state: State,
    planner: Planner,
    automation: Automation,
    /// Set when a tick changed the state, until [`Self::take_change`].
    changed: bool,
    /// No nudges until the next OFF (SPEC §13).
    power_only: bool,
    conditions: Conditions,
    /// The OS's time asleep at the last tick while ON or held.
    asleep_mark: Option<Duration>,
    events: VecDeque<Event>,
    /// The ON session so far, for the activity log (SPEC §15).
    session: Option<Session>,
    /// Gets each session as it ends. None while the log is off.
    session_sink: Option<SessionSink>,
}

/// Where finished sessions go: the app appends them to the activity log.
pub type SessionSink = Box<dyn FnMut(Session) + Send>;

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
            power_only: false,
            conditions: Conditions::default(),
            asleep_mark: None,
            events: VecDeque::with_capacity(EVENT_LOG_CAPACITY),
            session: None,
            session_sink: None,
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
        self.set_manual_with(on, false, now)
    }

    /// Like [`Self::set_manual_at`]. With `power_only`, an ON holds the power
    /// assertion without nudges until the next OFF; without it, nudges follow
    /// the settings again. The latest action decides.
    pub fn set_manual_with(
        &mut self,
        on: bool,
        power_only: bool,
        now: &Zoned,
    ) -> Result<(), AdapterError> {
        let previous = (self.automation.clone(), self.power_only);
        self.automation
            .set_manual(on, now.timestamp(), now.time_zone());
        if on {
            self.power_only = power_only;
        }
        let result = self.sync(now);
        if result.is_err() && on {
            // A failed ON leaves things as they were, not a pending ON.
            (self.automation, self.power_only) = previous;
        }
        result
    }

    /// ON until `until`, then back to the schedule (or OFF).
    pub fn start_timer(&mut self, until: Timestamp) -> Result<(), AdapterError> {
        self.start_timer_at(until, &Zoned::now())
    }

    pub fn start_timer_at(&mut self, until: Timestamp, now: &Zoned) -> Result<(), AdapterError> {
        self.start_timer_with(until, false, now)
    }

    /// Like [`Self::start_timer_at`], with `power_only` as in
    /// [`Self::set_manual_with`].
    pub fn start_timer_with(
        &mut self,
        until: Timestamp,
        power_only: bool,
        now: &Zoned,
    ) -> Result<(), AdapterError> {
        if until <= now.timestamp() {
            return Err(AdapterError("The timer must end in the future".into()));
        }
        let previous = (self.automation.clone(), self.power_only);
        self.automation.start_timer(until);
        self.power_only = power_only;
        let result = self.sync(now);
        if result.is_err() {
            (self.automation, self.power_only) = previous;
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

    /// Every session that ends from now on goes to `sink`, including the one
    /// running now. `None` stops that.
    pub fn set_session_sink(&mut self, sink: Option<SessionSink>) {
        self.session_sink = sink;
    }

    fn open_session(&mut self, source: Source, at: Timestamp) {
        self.session = Some(Session {
            start: at,
            end: at,
            source,
            power_only: self.power_only,
            nudges: 0,
        });
    }

    fn close_session(&mut self, at: Timestamp) {
        if let Some(mut session) = self.session.take() {
            session.end = at.max(session.start);
            if let Some(sink) = &mut self.session_sink {
                sink(session);
            }
        }
    }

    pub fn conditions(&self) -> &Conditions {
        &self.conditions
    }

    /// Takes effect at once: a condition that now holds pauses it, one that
    /// no longer does resumes it.
    pub fn set_conditions(&mut self, conditions: Conditions) -> Result<(), AdapterError> {
        self.conditions = conditions.cleaned();
        self.sync(&Zoned::now())
    }

    /// Reads only what the set conditions need.
    fn read_hold(&mut self) -> Option<Hold> {
        if !self.conditions.needs_watching() {
            return None;
        }
        let conditions = &self.conditions;
        let reading = Reading {
            locked: if conditions.release_when_locked {
                self.adapter.screen_locked()
            } else {
                None
            },
            power: if conditions.only_on_ac || conditions.min_battery.is_some() {
                self.adapter.power_source()
            } else {
                None
            },
            processes: if conditions.while_running.is_empty() {
                None
            } else {
                self.adapter.process_names()
            },
            networks: if conditions.on_networks.is_empty() {
                None
            } else {
                self.adapter.wifi_networks()
            },
            vpn: if conditions.only_on_vpn {
                self.adapter.vpn_connected()
            } else {
                None
            },
        };
        crate::conditions::hold(&self.conditions, &reading)
    }

    /// Brings the state in line with what the user, a timer or the schedule
    /// wants now, and what the conditions allow.
    fn sync(&mut self, now: &Zoned) -> Result<(), AdapterError> {
        let at = now.timestamp();
        self.automation.expire(at);
        let wanted = self.automation.decide(at, now.time_zone()).source;
        let hold = if wanted.is_some() {
            self.read_hold()
        } else {
            None
        };
        match (self.state, wanted, hold) {
            (State::Off, None, _) => {}
            (State::Held { .. }, None, _) => {
                self.state = State::Off;
                self.power_only = false;
            }
            (State::Off | State::Held { .. }, Some(_), Some(hold)) => {
                if !matches!(self.state, State::Held { hold: held } if held == hold) {
                    self.log(Instant::now(), EventKind::Held(hold));
                }
                self.state = State::Held { hold };
            }
            (State::On { .. }, Some(_), Some(hold)) => {
                self.state = State::Held { hold };
                self.planner.stop();
                self.log(Instant::now(), EventKind::Held(hold));
                self.close_session(at);
                self.adapter.release()?;
            }
            (State::Off | State::Held { .. }, Some(source), None) => {
                self.adapter.acquire()?;
                let started = Instant::now();
                self.state = State::On {
                    source,
                    since: started,
                };
                self.planner.start(started);
                self.log(started, EventKind::TurnedOn(source));
                self.open_session(source, at);
            }
            (State::On { source, since }, Some(wanted), None) => {
                // `unidle on` while ON makes the rest of it power-only.
                if let Some(session) = &mut self.session {
                    session.power_only |= self.power_only;
                }
                if source != wanted {
                    self.state = State::On {
                        source: wanted,
                        since,
                    };
                    self.log(Instant::now(), EventKind::SourceChanged(wanted));
                }
            }
            (State::On { .. }, None, _) => {
                // OFF wins even if the release fails: the OS drops the
                // assertion when the process exits, and the UI must not
                // claim to be ON.
                self.state = State::Off;
                self.power_only = false;
                self.planner.stop();
                self.log(Instant::now(), EventKind::TurnedOff);
                self.close_session(at);
                self.adapter.release()?;
            }
        }
        Ok(())
    }

    pub fn status(&self) -> Status {
        self.status_at(&Zoned::now())
    }

    pub fn status_at(&self, now: &Zoned) -> Status {
        let (source, elapsed, held) = match self.state {
            State::Off => (None, Duration::ZERO, None),
            State::On { source, since } => (Some(source), since.elapsed(), None),
            State::Held { hold, .. } => (None, Duration::ZERO, Some(hold)),
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
            power_only: source.is_some() && self.power_only,
            held,
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
        self.check_slept(now, wall);
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
        let mut wait = self.nudge_wait(now);
        if let Some(watch_wait) = self.watch_wait(wall) {
            wait = wait.min(watch_wait);
        }
        if !matches!(self.state, State::Off) {
            if self.conditions.needs_watching() {
                wait = wait.min(CONDITION_WAIT);
            } else if self.conditions.off_after_sleep {
                // Only the sleep check: the ticker's clock stops while asleep,
                // so this bounds how long after waking it is noticed.
                wait = wait.min(WATCH_WAIT);
            }
        }
        wait
    }

    /// The computer slept while ON or held: the user chose Sleep or closed
    /// the lid, since the assertion blocks idle sleep. Turn OFF as a manual
    /// OFF would, instead of carrying on after waking (SPEC §14).
    fn check_slept(&mut self, now: Instant, wall: &Zoned) {
        if matches!(self.state, State::Off) || !self.conditions.off_after_sleep {
            self.asleep_mark = None;
            return;
        }
        let asleep = self.adapter.time_asleep();
        if let (Some(before), Some(after)) = (self.asleep_mark, asleep)
            && after.saturating_sub(before) >= SLEPT
        {
            self.automation
                .set_manual(false, wall.timestamp(), wall.time_zone());
            self.log(now, EventKind::OffAfterSleep);
            // The session ended as the computer went to sleep, not on waking.
            let slept = after.saturating_sub(before);
            let fell_asleep = jiff::SignedDuration::try_from(slept)
                .ok()
                .and_then(|slept| wall.timestamp().checked_sub(slept).ok())
                .unwrap_or_else(|| wall.timestamp());
            self.close_session(fell_asleep);
        }
        self.asleep_mark = asleep;
    }

    /// Whether a tick changed the state since the last call, so the UI can
    /// catch up with a timer or schedule.
    pub fn take_change(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    fn state_key(&self) -> (Option<Source>, Option<Hold>) {
        match self.state {
            State::Off => (None, None),
            State::On { source, .. } => (Some(source), None),
            State::Held { hold, .. } => (None, Some(hold)),
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

        // Nudging a locked screen does nothing for anyone (SPEC §14).
        if self.adapter.screen_locked() == Some(true) {
            self.planner.start(now);
            self.log(now, EventKind::SkippedLocked);
            return self
                .planner
                .next_at()
                .map_or(IDLE_WAIT, |next| next.saturating_duration_since(now));
        }
        let idle = self.adapter.idle_time();
        let kind = match self.planner.due(now, idle) {
            Due::SkipUserActive => EventKind::SkippedUserActive,
            Due::Nudge(kind) => match self.adapter.nudge(kind) {
                Ok(()) => {
                    if let Some(session) = &mut self.session {
                        session.nudges += 1;
                    }
                    EventKind::Nudged(kind)
                }
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
        self.planner.config().enabled && capabilities.input_nudge && !self.power_only
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
        self.close_session(Timestamp::now());
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
        env: Rc<RefCell<FakeEnv>>,
    }

    #[derive(Default)]
    struct FakeEnv {
        locked: Option<bool>,
        power: Option<crate::platform::PowerSource>,
        processes: Option<Vec<String>>,
        asleep: Option<Duration>,
        networks: Option<Vec<String>>,
        vpn: Option<bool>,
    }

    impl Default for FakeAdapter {
        fn default() -> Self {
            Self {
                calls: Rc::default(),
                fail_acquire: false,
                fail_release: false,
                input_nudge: true,
                idle: None,
                env: Rc::default(),
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

        fn screen_locked(&mut self) -> Option<bool> {
            self.env.borrow().locked
        }

        fn power_source(&mut self) -> Option<crate::platform::PowerSource> {
            self.env.borrow().power
        }

        fn process_names(&mut self) -> Option<Vec<String>> {
            self.env.borrow().processes.clone()
        }

        fn time_asleep(&mut self) -> Option<Duration> {
            self.env.borrow().asleep
        }

        fn wifi_networks(&mut self) -> Option<Vec<String>> {
            self.env.borrow().networks.clone()
        }

        fn vpn_connected(&mut self) -> Option<bool> {
            self.env.borrow().vpn
        }
    }

    mod conditions {
        use super::*;
        use crate::conditions::{Conditions, Hold};
        use crate::platform::PowerSource;

        fn battery(on_ac: bool) -> Option<PowerSource> {
            Some(PowerSource {
                on_ac,
                battery_percent: Some(50),
            })
        }

        #[test]
        fn on_battery_holds_and_resumes_when_plugged_in() {
            let adapter = FakeAdapter::default();
            let (calls, env) = (adapter.calls.clone(), adapter.env.clone());
            env.borrow_mut().power = battery(false);
            let mut controller = Controller::new(adapter);
            controller
                .set_conditions(Conditions {
                    only_on_ac: true,
                    ..Conditions::new()
                })
                .unwrap();

            controller.turn_on().unwrap();
            let status = controller.status();
            assert!(!status.is_on());
            assert_eq!(status.held, Some(Hold::OnBattery));
            assert_eq!(calls.borrow().acquire, 0);

            // Held: the ticker comes back soon to look again.
            let wait = controller.tick(Instant::now());
            assert!(wait <= Duration::from_secs(5));

            env.borrow_mut().power = battery(true);
            controller.tick(Instant::now());
            assert!(controller.status().is_on());
            assert!(controller.take_change());
            assert_eq!(calls.borrow().acquire, 1);

            // Unplugged again while ON: released, held.
            env.borrow_mut().power = battery(false);
            controller.tick(Instant::now());
            assert_eq!(controller.status().held, Some(Hold::OnBattery));
            assert_eq!(calls.borrow().release, 1);

            // OFF while held is plain OFF.
            controller.turn_off().unwrap();
            let status = controller.status();
            assert!(!status.is_on() && status.held.is_none());
            assert_eq!(calls.borrow().release, 1);
        }

        #[test]
        fn app_condition_follows_the_process_list() {
            let adapter = FakeAdapter::default();
            let env = adapter.env.clone();
            env.borrow_mut().processes = Some(vec!["explorer".into()]);
            let mut controller = Controller::new(adapter);
            controller
                .set_conditions(Conditions {
                    while_running: vec!["Blender.exe".into()],
                    ..Conditions::new()
                })
                .unwrap();
            controller.turn_on().unwrap();
            assert_eq!(controller.status().held, Some(Hold::AppNotRunning));
            env.borrow_mut().processes = Some(vec!["explorer".into(), "blender".into()]);
            controller.tick(Instant::now());
            assert!(controller.status().is_on());
            // Removing the condition while held resumes at once.
            env.borrow_mut().processes = Some(vec![]);
            controller.tick(Instant::now());
            assert!(!controller.status().is_on());
            controller.set_conditions(Conditions::new()).unwrap();
            assert!(controller.status().is_on());
        }

        #[test]
        fn network_conditions_hold_and_resume() {
            let adapter = FakeAdapter::default();
            let env = adapter.env.clone();
            env.borrow_mut().networks = Some(vec!["cafe".into()]);
            env.borrow_mut().vpn = Some(false);
            let mut controller = Controller::new(adapter);
            controller
                .set_conditions(Conditions {
                    on_networks: vec!["Office".into()],
                    only_on_vpn: true,
                    ..Conditions::new()
                })
                .unwrap();
            controller.turn_on().unwrap();
            assert_eq!(controller.status().held, Some(Hold::NotOnNetwork));
            env.borrow_mut().networks = Some(vec!["office".into()]);
            controller.tick(Instant::now());
            assert_eq!(controller.status().held, Some(Hold::NoVpn));
            env.borrow_mut().vpn = Some(true);
            controller.tick(Instant::now());
            assert!(controller.status().is_on());
        }

        #[test]
        fn a_locked_screen_gets_no_nudges_but_stays_awake() {
            let adapter = FakeAdapter::default();
            let (calls, env) = (adapter.calls.clone(), adapter.env.clone());
            env.borrow_mut().locked = Some(true);
            let mut controller = Controller::new(adapter);
            controller.turn_on().unwrap();
            let start = Instant::now();
            controller.tick(start + Duration::from_secs(73));
            assert!(calls.borrow().nudges.is_empty());
            assert!(controller.status().is_on());
            assert!(
                controller
                    .events()
                    .any(|event| event.kind == EventKind::SkippedLocked)
            );

            env.borrow_mut().locked = Some(false);
            controller.tick(start + Duration::from_secs(73 + 73));
            assert_eq!(calls.borrow().nudges.len(), 1);
        }

        #[test]
        fn release_when_locked_lets_it_sleep_until_unlocked() {
            let adapter = FakeAdapter::default();
            let (calls, env) = (adapter.calls.clone(), adapter.env.clone());
            let mut controller = Controller::new(adapter);
            controller
                .set_conditions(Conditions {
                    release_when_locked: true,
                    ..Conditions::new()
                })
                .unwrap();
            controller.turn_on().unwrap();
            env.borrow_mut().locked = Some(true);
            controller.tick(Instant::now());
            assert_eq!(controller.status().held, Some(Hold::Locked));
            assert_eq!(calls.borrow().release, 1);
            env.borrow_mut().locked = Some(false);
            controller.tick(Instant::now());
            assert!(controller.status().is_on());
        }

        #[test]
        fn sleeping_while_on_turns_it_off() {
            let adapter = FakeAdapter::default();
            let env = adapter.env.clone();
            env.borrow_mut().asleep = Some(Duration::from_secs(100));
            let mut controller = Controller::new(adapter);
            controller.turn_on().unwrap();
            controller.tick(Instant::now());
            // A short blip isn't sleep.
            env.borrow_mut().asleep = Some(Duration::from_secs(105));
            controller.tick(Instant::now());
            assert!(controller.status().is_on());

            env.borrow_mut().asleep = Some(Duration::from_secs(105 + 3600));
            controller.tick(Instant::now());
            assert!(!controller.status().is_on());
            assert!(controller.take_change());
            assert!(
                controller
                    .events()
                    .any(|event| event.kind == EventKind::OffAfterSleep)
            );

            // Unless asked not to.
            controller
                .set_conditions(Conditions {
                    off_after_sleep: false,
                    ..Conditions::new()
                })
                .unwrap();
            controller.turn_on().unwrap();
            controller.tick(Instant::now());
            env.borrow_mut().asleep = Some(Duration::from_secs(105 + 7200));
            controller.tick(Instant::now());
            assert!(controller.status().is_on());
        }

        #[test]
        fn sleeping_during_a_schedule_window_pauses_it_like_a_manual_off() {
            use crate::schedule::{Schedule, TimeWindow, Weekdays};
            let adapter = FakeAdapter::default();
            let env = adapter.env.clone();
            env.borrow_mut().asleep = Some(Duration::ZERO);
            let mut controller = Controller::new(adapter);
            let now = Zoned::now();
            let schedule = Schedule {
                enabled: true,
                windows: vec![
                    TimeWindow::new(Weekdays::empty().with(now.weekday()), 0, 24 * 60).unwrap(),
                ],
                ..Schedule::default()
            };
            controller.set_schedule_at(schedule, &now).unwrap();
            assert_eq!(controller.status().source, Some(Source::Schedule));
            controller.tick(Instant::now());
            env.borrow_mut().asleep = Some(Duration::from_secs(600));
            controller.tick(Instant::now());
            let status = controller.status();
            assert!(!status.is_on());
            assert!(status.paused_until.is_some());
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
        // Only the nudge decides the wait here, not the sleep check.
        controller
            .set_conditions(crate::conditions::Conditions {
                off_after_sleep: false,
                ..Default::default()
            })
            .unwrap();

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
    fn power_only_on_sends_no_nudges_until_the_next_off() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let mut controller = Controller::new(adapter);
        let now = Zoned::now();

        controller.set_manual_with(true, true, &now).unwrap();
        let start = Instant::now();
        controller.tick(start + Duration::from_secs(600));
        assert!(calls.borrow().nudges.is_empty());
        let status = controller.status();
        assert!(status.is_on() && status.power_only && !status.nudging);
        assert_eq!(calls.borrow().acquire, 1);

        // OFF clears it: the next ON follows the settings.
        controller.turn_off().unwrap();
        assert!(!controller.status().power_only);
        controller.turn_on().unwrap();
        assert!(controller.status().nudging);
        controller.tick(Instant::now() + Duration::from_secs(73));
        assert_eq!(calls.borrow().nudges.len(), 1);
    }

    #[test]
    fn the_latest_on_decides_power_only() {
        let mut controller = Controller::new(FakeAdapter::default());
        let now = Zoned::now();
        controller.turn_on().unwrap();
        controller.set_manual_with(true, true, &now).unwrap();
        assert!(controller.status().power_only);
        controller.set_manual_with(true, false, &now).unwrap();
        assert!(!controller.status().power_only);

        let until = now.timestamp() + jiff::SignedDuration::from_mins(30);
        controller.start_timer_with(until, true, &now).unwrap();
        let status = controller.status();
        assert_eq!(status.source, Some(Source::Timer));
        assert!(status.power_only && !status.nudging);
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

    mod sessions {
        use std::sync::{Arc, Mutex};

        use super::*;
        use crate::conditions::Conditions;
        use crate::schedule::tests::{at, paris};

        fn wall(time: &str) -> Zoned {
            at(time).to_zoned(paris())
        }

        fn logged(controller: &mut Controller<FakeAdapter>) -> Arc<Mutex<Vec<Session>>> {
            let sessions: Arc<Mutex<Vec<Session>>> = Arc::default();
            let sink = sessions.clone();
            controller.set_session_sink(Some(Box::new(move |session| {
                sink.lock().unwrap().push(session);
            })));
            sessions
        }

        #[test]
        fn a_session_runs_from_on_to_off_and_counts_nudges() {
            let mut controller = Controller::new(FakeAdapter::default());
            let sessions = logged(&mut controller);
            controller
                .set_manual_at(true, &wall("2026-11-05T09:00"))
                .unwrap();
            controller.tick_at(
                Instant::now() + Duration::from_secs(73),
                &wall("2026-11-05T09:01"),
            );
            assert!(sessions.lock().unwrap().is_empty());
            controller
                .set_manual_at(false, &wall("2026-11-05T10:00"))
                .unwrap();
            assert_eq!(
                *sessions.lock().unwrap(),
                [Session {
                    start: at("2026-11-05T09:00"),
                    end: at("2026-11-05T10:00"),
                    source: Source::Manual,
                    power_only: false,
                    nudges: 1,
                }]
            );
        }

        #[test]
        fn the_running_session_is_logged_and_power_only_sticks() {
            let mut controller = Controller::new(FakeAdapter::default());
            controller.turn_on().unwrap();
            let sessions = logged(&mut controller);
            // The session running when the log is turned on is logged too.
            controller
                .set_manual_with(true, true, &wall("2026-11-05T09:30"))
                .unwrap();
            controller.turn_off().unwrap();
            let sessions = sessions.lock().unwrap();
            assert_eq!(sessions.len(), 1);
            assert!(sessions[0].power_only);
            assert_eq!(sessions[0].nudges, 0);
        }

        #[test]
        fn a_hold_ends_the_session_and_resuming_starts_another() {
            let adapter = FakeAdapter::default();
            let env = adapter.env.clone();
            env.borrow_mut().locked = Some(false);
            let mut controller = Controller::new(adapter);
            let sessions = logged(&mut controller);
            controller
                .set_conditions(Conditions {
                    release_when_locked: true,
                    ..Conditions::new()
                })
                .unwrap();
            controller
                .set_manual_at(true, &wall("2026-11-05T09:00"))
                .unwrap();
            env.borrow_mut().locked = Some(true);
            controller.tick_at(Instant::now(), &wall("2026-11-05T09:20"));
            env.borrow_mut().locked = Some(false);
            controller.tick_at(Instant::now(), &wall("2026-11-05T09:50"));
            controller
                .set_manual_at(false, &wall("2026-11-05T10:00"))
                .unwrap();
            let spans: Vec<_> = sessions
                .lock()
                .unwrap()
                .iter()
                .map(|session| (session.start, session.end))
                .collect();
            assert_eq!(
                spans,
                [
                    (at("2026-11-05T09:00"), at("2026-11-05T09:20")),
                    (at("2026-11-05T09:50"), at("2026-11-05T10:00")),
                ]
            );
        }

        #[test]
        fn sleep_ends_the_session_when_the_computer_fell_asleep() {
            let adapter = FakeAdapter::default();
            let env = adapter.env.clone();
            env.borrow_mut().asleep = Some(Duration::from_secs(100));
            let mut controller = Controller::new(adapter);
            let sessions = logged(&mut controller);
            controller
                .set_manual_at(true, &wall("2026-11-05T09:00"))
                .unwrap();
            controller.tick_at(Instant::now(), &wall("2026-11-05T09:10"));
            env.borrow_mut().asleep = Some(Duration::from_secs(100 + 3600));
            controller.tick_at(Instant::now(), &wall("2026-11-05T11:10"));
            assert!(!controller.status().is_on());
            let sessions = sessions.lock().unwrap();
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].end, at("2026-11-05T10:10"));
        }

        #[test]
        fn quitting_ends_the_session() {
            let mut controller = Controller::new(FakeAdapter::default());
            let sessions = logged(&mut controller);
            controller.turn_on().unwrap();
            drop(controller);
            assert_eq!(sessions.lock().unwrap().len(), 1);
        }
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

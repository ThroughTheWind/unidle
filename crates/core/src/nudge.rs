//! When to nudge, and how: interval with jitter, method alternation, and
//! skipping while the user is active (SPEC FR-7, FR-8). No OS calls here.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::adapter::NudgeKind;

pub const MIN_INTERVAL: Duration = Duration::from_secs(10);
pub const MAX_INTERVAL: Duration = Duration::from_secs(600);
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);
/// Each delay is the interval ±20% (SPEC §4).
const JITTER: f64 = 0.2;
/// Input seen this soon after our own nudge is taken to be the nudge itself.
pub(crate) const OWN_INPUT_TOLERANCE: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NudgeMethod {
    #[default]
    Pointer,
    Keystroke,
    /// Alternate pointer and keystroke nudges.
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NudgeConfig {
    pub enabled: bool,
    pub method: NudgeMethod,
    /// Clamped to `MIN_INTERVAL..=MAX_INTERVAL`.
    pub interval: Duration,
}

impl Default for NudgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            method: NudgeMethod::default(),
            interval: DEFAULT_INTERVAL,
        }
    }
}

impl NudgeConfig {
    pub fn clamped(self) -> Self {
        Self {
            interval: self.interval.clamp(MIN_INTERVAL, MAX_INTERVAL),
            ..self
        }
    }
}

/// What the planner decided when a nudge came due.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Due {
    Nudge(NudgeKind),
    /// Real input happened during this interval (SPEC FR-8).
    SkipUserActive,
}

/// Small xorshift generator: jitter needs to be unpredictable to a presence
/// tracker, not cryptographically random, and this avoids a dependency.
#[derive(Clone, Debug)]
struct Rng(u64);

impl Rng {
    fn seeded() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self(nanos | 1)
    }

    /// Uniform in `[0, 1)`.
    fn next_unit(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Planner {
    config: NudgeConfig,
    rng: Rng,
    /// Start of the current interval: when we turned ON or last nudged or skipped.
    interval_start: Option<Instant>,
    next_at: Option<Instant>,
    next_kind: NudgeKind,
}

impl Planner {
    pub(crate) fn new(config: NudgeConfig) -> Self {
        Self::with_seed(config, Rng::seeded())
    }

    fn with_seed(config: NudgeConfig, rng: Rng) -> Self {
        let config = config.clamped();
        Self {
            config,
            rng,
            interval_start: None,
            next_at: None,
            next_kind: first_kind(config.method),
        }
    }

    pub(crate) fn config(&self) -> NudgeConfig {
        self.config
    }

    pub(crate) fn set_config(&mut self, config: NudgeConfig, now: Instant) {
        let config = config.clamped();
        if config == self.config {
            return;
        }
        self.config = config;
        self.next_kind = first_kind(config.method);
        if self.interval_start.is_some() {
            self.start(now);
        }
    }

    pub(crate) fn start(&mut self, now: Instant) {
        self.interval_start = Some(now);
        self.next_at = Some(now + self.delay());
    }

    pub(crate) fn stop(&mut self) {
        self.interval_start = None;
        self.next_at = None;
    }

    pub(crate) fn next_at(&self) -> Option<Instant> {
        self.next_at
    }

    /// Call when `next_at` has passed. `idle` is the OS's time since the last
    /// input. Starts the next interval.
    pub(crate) fn due(&mut self, now: Instant, idle: Option<Duration>) -> Due {
        let since_start = self
            .interval_start
            .map_or(Duration::ZERO, |start| now.saturating_duration_since(start));
        // Our own nudge at the start of the interval resets the OS idle
        // timer too, so only input clearly after it counts as the user's.
        let user_active = idle.is_some_and(|idle| idle + OWN_INPUT_TOLERANCE < since_start);

        self.start(now);
        if user_active {
            return Due::SkipUserActive;
        }
        let kind = self.next_kind;
        if self.config.method == NudgeMethod::Both {
            self.next_kind = match kind {
                NudgeKind::Pointer => NudgeKind::Keystroke,
                NudgeKind::Keystroke => NudgeKind::Pointer,
            };
        }
        Due::Nudge(kind)
    }

    fn delay(&mut self) -> Duration {
        let factor = 1.0 + JITTER * (2.0 * self.rng.next_unit() - 1.0);
        self.config.interval.mul_f64(factor)
    }
}

fn first_kind(method: NudgeMethod) -> NudgeKind {
    match method {
        NudgeMethod::Keystroke => NudgeKind::Keystroke,
        NudgeMethod::Pointer | NudgeMethod::Both => NudgeKind::Pointer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planner(method: NudgeMethod) -> Planner {
        Planner::with_seed(
            NudgeConfig {
                method,
                ..NudgeConfig::default()
            },
            Rng(42),
        )
    }

    #[test]
    fn delays_stay_within_twenty_percent_of_the_interval() {
        let mut planner = planner(NudgeMethod::Pointer);
        let now = Instant::now();
        let (mut low, mut high) = (Duration::MAX, Duration::ZERO);
        for _ in 0..10_000 {
            planner.start(now);
            let delay = planner.next_at().unwrap() - now;
            low = low.min(delay);
            high = high.max(delay);
        }
        assert!(low >= Duration::from_secs(48), "{low:?}");
        assert!(high <= Duration::from_secs(72), "{high:?}");
        // The jitter actually varies.
        assert!(high - low > Duration::from_secs(20), "{low:?}..{high:?}");
    }

    #[test]
    fn interval_is_clamped_to_the_spec_range() {
        let short = NudgeConfig {
            interval: Duration::from_secs(1),
            ..NudgeConfig::default()
        };
        let long = NudgeConfig {
            interval: Duration::from_secs(3600),
            ..NudgeConfig::default()
        };
        assert_eq!(short.clamped().interval, MIN_INTERVAL);
        assert_eq!(long.clamped().interval, MAX_INTERVAL);
    }

    #[test]
    fn both_alternates_pointer_and_keystroke() {
        let mut planner = planner(NudgeMethod::Both);
        let start = Instant::now();
        planner.start(start);
        let kinds: Vec<_> = (1..=4)
            .map(|step| {
                let now = start + Duration::from_secs(60 * step);
                // Idle since our previous nudge: no real input.
                planner.due(now, Some(Duration::from_secs(60)))
            })
            .collect();
        assert_eq!(
            kinds,
            [
                Due::Nudge(NudgeKind::Pointer),
                Due::Nudge(NudgeKind::Keystroke),
                Due::Nudge(NudgeKind::Pointer),
                Due::Nudge(NudgeKind::Keystroke),
            ]
        );
    }

    #[test]
    fn keystroke_method_only_sends_keystrokes() {
        let mut planner = planner(NudgeMethod::Keystroke);
        let start = Instant::now();
        planner.start(start);
        for step in 1..=3 {
            let now = start + Duration::from_secs(60 * step);
            assert_eq!(planner.due(now, None), Due::Nudge(NudgeKind::Keystroke));
        }
    }

    #[test]
    fn real_input_during_the_interval_skips_the_nudge() {
        let mut planner = planner(NudgeMethod::Pointer);
        let start = Instant::now();
        planner.start(start);
        // The user typed 5 s ago, well after the interval began.
        let now = start + Duration::from_secs(60);
        assert_eq!(
            planner.due(now, Some(Duration::from_secs(5))),
            Due::SkipUserActive
        );
    }

    #[test]
    fn our_own_nudge_does_not_count_as_user_input() {
        let mut planner = planner(NudgeMethod::Pointer);
        let start = Instant::now();
        planner.start(start);
        // The only input is our nudge at the start of the interval, a few
        // milliseconds after the planner's clock started it.
        let now = start + Duration::from_secs(60);
        let idle = Duration::from_secs(60) - Duration::from_millis(30);
        assert_eq!(planner.due(now, Some(idle)), Due::Nudge(NudgeKind::Pointer));
    }

    #[test]
    fn unknown_idle_time_never_skips() {
        let mut planner = planner(NudgeMethod::Pointer);
        let start = Instant::now();
        planner.start(start);
        assert_eq!(
            planner.due(start + Duration::from_secs(60), None),
            Due::Nudge(NudgeKind::Pointer)
        );
    }

    #[test]
    fn changing_the_config_while_running_restarts_the_interval() {
        let mut planner = planner(NudgeMethod::Pointer);
        let start = Instant::now();
        planner.start(start);
        let later = start + Duration::from_secs(30);
        planner.set_config(
            NudgeConfig {
                interval: Duration::from_secs(600),
                ..NudgeConfig::default()
            },
            later,
        );
        assert!(planner.next_at().unwrap() >= later + Duration::from_secs(480));
    }
}

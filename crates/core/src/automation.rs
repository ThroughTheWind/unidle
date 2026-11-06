//! Who decides whether simulation is ON: the user, a timer, the schedule or
//! the calendar's meetings (SPEC §10, §18). Pure decisions from a wall-clock time, so the
//! answer is right after a sleep or a DST change without catching up on
//! anything.
//!
//! Precedence:
//! - The latest manual action or timer wins over the schedule. Manual and
//!   timer replace each other, so the most recent one counts.
//! - A manual ON or OFF that differs from the schedule holds until the
//!   schedule next changes. Turning OFF during a window pauses it until the
//!   window ends; turning ON outside one keeps it on until the next window
//!   starts, and the schedule takes over from there. Without a schedule, a
//!   manual change holds until the next one.
//! - A timer keeps it ON until it ends, then the schedule decides again.
//! - The calendar's meetings plan ON times just as schedule windows do, and
//!   the two together are "the plan" below. A schedule window wins the
//!   source when both run.

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::controller::Source;
use crate::schedule::Schedule;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Override {
    Manual {
        on: bool,
        /// `None` holds until the next manual action.
        until: Option<Timestamp>,
    },
    Timer {
        until: Timestamp,
    },
}

impl Override {
    fn until(self) -> Option<Timestamp> {
        match self {
            Self::Manual { until, .. } => until,
            Self::Timer { until } => Some(until),
        }
    }

    fn is_active(self, at: Timestamp) -> bool {
        self.until().is_none_or(|until| at < until)
    }
}

/// What simulation should be at a given time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// `None` is OFF.
    pub source: Option<Source>,
}

impl Decision {
    pub fn is_on(self) -> bool {
        self.source.is_some()
    }
}

/// The next time the app turns itself ON or OFF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextChange {
    pub at: Timestamp,
    /// What it turns to.
    pub on: bool,
    /// What turns it ON then; `None` for an OFF.
    pub source: Option<Source>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Automation {
    schedule: Schedule,
    /// Busy times from the calendar, sorted and merged.
    meetings: Vec<(Timestamp, Timestamp)>,
    held: Option<Override>,
}

impl Automation {
    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    pub fn meetings(&self) -> &[(Timestamp, Timestamp)] {
        &self.meetings
    }

    /// Whether anything will change on its own, so the time must be watched.
    pub fn is_watching(&self) -> bool {
        self.schedule.is_active()
            || !self.meetings.is_empty()
            || self.held.is_some_and(|held| held.until().is_some())
    }

    pub fn decide(&self, at: Timestamp, tz: &TimeZone) -> Decision {
        let source = match self.held.filter(|held| held.is_active(at)) {
            Some(Override::Manual { on, .. }) => on.then_some(Source::Manual),
            Some(Override::Timer { .. }) => Some(Source::Timer),
            None => self.planned(at, tz),
        };
        Decision { source }
    }

    /// What the schedule and the calendar want at `at`.
    fn planned(&self, at: Timestamp, tz: &TimeZone) -> Option<Source> {
        if self.schedule.is_on(at, tz) {
            Some(Source::Schedule)
        } else if self.in_meeting(at) {
            Some(Source::Calendar)
        } else {
            None
        }
    }

    fn in_meeting(&self, at: Timestamp) -> bool {
        // Sorted and merged: only the last meeting that starts by `at` can
        // contain it.
        let index = self.meetings.partition_point(|&(start, _)| start <= at);
        index > 0 && at < self.meetings[index - 1].1
    }

    /// Every schedule boundary and meeting start and end after `at`, in
    /// order, without duplicates.
    fn boundaries_after(&self, at: Timestamp, tz: &TimeZone) -> Vec<Timestamp> {
        let mut boundaries = self.schedule.boundaries_after(at, tz);
        boundaries.extend(
            self.meetings
                .iter()
                .flat_map(|&(start, end)| [start, end])
                .filter(|&boundary| boundary > at),
        );
        boundaries.sort_unstable();
        boundaries.dedup();
        boundaries
    }

    /// The first time after `at` when the plan turns ON or OFF.
    fn next_plan_flip(&self, at: Timestamp, tz: &TimeZone) -> Option<Timestamp> {
        let on = self.planned(at, tz).is_some();
        self.boundaries_after(at, tz)
            .into_iter()
            .find(|&boundary| self.planned(boundary, tz).is_some() != on)
    }

    /// Forgets an override that has ended.
    pub fn expire(&mut self, at: Timestamp) {
        if self.held.is_some_and(|held| !held.is_active(at)) {
            self.held = None;
        }
    }

    /// The user turned it ON or OFF. Cancels a running timer.
    pub fn set_manual(&mut self, on: bool, at: Timestamp, tz: &TimeZone) {
        self.held = if self.planned(at, tz).is_some() == on {
            // Back in line with the plan: follow it again.
            None
        } else {
            Some(Override::Manual {
                on,
                until: self.next_plan_flip(at, tz),
            })
        };
    }

    /// ON until `until`. Replaces a manual override or an earlier timer.
    pub fn start_timer(&mut self, until: Timestamp) {
        self.held = Some(Override::Timer { until });
    }

    #[cfg(test)]
    fn timer_until(&self, at: Timestamp) -> Option<Timestamp> {
        match self.held.filter(|held| held.is_active(at)) {
            Some(Override::Timer { until }) => Some(until),
            _ => None,
        }
    }

    /// When a manual OFF pauses a schedule window or a meeting: the time the
    /// pause ends, and which of the two it paused.
    pub fn paused(&self, at: Timestamp, tz: &TimeZone) -> Option<(Timestamp, Source)> {
        match self.held.filter(|held| held.is_active(at)) {
            Some(Override::Manual {
                on: false,
                until: Some(until),
            }) => self.planned(at, tz).map(|source| (until, source)),
            _ => None,
        }
    }

    /// Replaces the schedule. A manual override is re-anchored on the new
    /// plan's next change, as if it had just been made.
    pub fn set_schedule(&mut self, schedule: Schedule, at: Timestamp, tz: &TimeZone) {
        self.schedule = schedule;
        self.re_anchor(at, tz);
    }

    /// Replaces the calendar's busy times, sorted and merged. A manual
    /// override is re-anchored as for [`Self::set_schedule`].
    pub fn set_meetings(
        &mut self,
        meetings: Vec<(Timestamp, Timestamp)>,
        at: Timestamp,
        tz: &TimeZone,
    ) {
        self.meetings = crate::calendar::merge(meetings);
        self.re_anchor(at, tz);
    }

    fn re_anchor(&mut self, at: Timestamp, tz: &TimeZone) {
        if let Some(Override::Manual { on, .. }) = self.held.filter(|held| held.is_active(at)) {
            self.set_manual(on, at, tz);
        }
    }

    /// The first time after `at` when the decision turns ON or OFF.
    pub fn next_change(&self, at: Timestamp, tz: &TimeZone) -> Option<NextChange> {
        let on = self.decide(at, tz).is_on();
        let mut candidates = self.boundaries_after(at, tz);
        if let Some(until) = self
            .held
            .and_then(Override::until)
            .filter(|&until| until > at)
        {
            candidates.push(until);
            candidates.sort_unstable();
        }
        candidates
            .into_iter()
            .map(|candidate| (candidate, self.decide(candidate, tz).source))
            .find(|&(_, source)| source.is_some() != on)
            .map(|(at, source)| NextChange {
                at,
                on: !on,
                source,
            })
    }
}

#[cfg(test)]
mod tests {
    use jiff::SignedDuration;

    use super::*;
    use crate::schedule::tests::{at, office_hours, paris};

    fn automation() -> Automation {
        let mut automation = Automation::default();
        automation.set_schedule(office_hours(), at("2026-11-02T00:00"), &paris());
        automation
    }

    fn source(automation: &Automation, time: &str) -> Option<Source> {
        automation.decide(at(time), &paris()).source
    }

    #[test]
    fn follows_the_schedule_by_default() {
        let automation = automation();
        assert_eq!(source(&automation, "2026-11-02T08:00"), None);
        assert_eq!(
            source(&automation, "2026-11-02T10:00"),
            Some(Source::Schedule)
        );
        assert_eq!(
            automation.next_change(at("2026-11-02T10:00"), &paris()),
            Some(NextChange {
                at: at("2026-11-02T18:00"),
                on: false,
                source: None,
            })
        );
    }

    #[test]
    fn manual_off_pauses_the_window_until_it_ends() {
        let mut automation = automation();
        let tz = paris();
        automation.set_manual(false, at("2026-11-02T10:00"), &tz);

        assert_eq!(source(&automation, "2026-11-02T17:00"), None);
        assert_eq!(
            automation.paused(at("2026-11-02T17:00"), &tz),
            Some((at("2026-11-02T18:00"), Source::Schedule))
        );
        // Tuesday's window runs as usual.
        assert_eq!(
            automation.next_change(at("2026-11-02T17:00"), &tz),
            Some(NextChange {
                at: at("2026-11-03T09:00"),
                on: true,
                source: Some(Source::Schedule),
            })
        );
        assert_eq!(
            source(&automation, "2026-11-03T09:00"),
            Some(Source::Schedule)
        );
    }

    #[test]
    fn manual_on_outside_a_window_hands_over_at_the_next_one() {
        let mut automation = automation();
        let tz = paris();
        automation.set_manual(true, at("2026-11-02T20:00"), &tz);

        assert_eq!(
            source(&automation, "2026-11-03T08:59"),
            Some(Source::Manual)
        );
        assert_eq!(
            source(&automation, "2026-11-03T09:00"),
            Some(Source::Schedule)
        );
        // ON all along, so the next change is the window's end.
        assert_eq!(
            automation.next_change(at("2026-11-02T20:00"), &tz),
            Some(NextChange {
                at: at("2026-11-03T18:00"),
                on: false,
                source: None,
            })
        );
    }

    #[test]
    fn manual_on_inside_a_paused_window_resumes_the_schedule() {
        let mut automation = automation();
        let tz = paris();
        automation.set_manual(false, at("2026-11-02T10:00"), &tz);
        automation.set_manual(true, at("2026-11-02T11:00"), &tz);
        assert_eq!(
            source(&automation, "2026-11-02T11:00"),
            Some(Source::Schedule)
        );
    }

    #[test]
    fn without_a_schedule_manual_holds_until_the_next_action() {
        let mut automation = Automation::default();
        let tz = paris();
        automation.set_manual(true, at("2026-11-02T10:00"), &tz);
        assert_eq!(
            source(&automation, "2027-01-01T00:00"),
            Some(Source::Manual)
        );
        assert!(!automation.is_watching());
        assert_eq!(automation.next_change(at("2026-11-02T10:00"), &tz), None);
        automation.set_manual(false, at("2026-11-02T11:00"), &tz);
        assert_eq!(source(&automation, "2026-11-02T11:00"), None);
    }

    #[test]
    fn a_timer_runs_out_then_the_schedule_decides() {
        let tz = paris();
        // Outside a window: OFF when the timer ends.
        let mut automation = automation();
        let start = at("2026-11-02T19:00");
        automation.start_timer(start + SignedDuration::from_mins(30));
        assert_eq!(source(&automation, "2026-11-02T19:29"), Some(Source::Timer));
        assert_eq!(
            automation.next_change(start, &tz),
            Some(NextChange {
                at: at("2026-11-02T19:30"),
                on: false,
                source: None,
            })
        );
        assert_eq!(source(&automation, "2026-11-02T19:30"), None);

        // Running into a window: the schedule keeps it ON.
        let mut automation = self::automation();
        automation.start_timer(at("2026-11-02T09:30"));
        assert_eq!(source(&automation, "2026-11-02T09:15"), Some(Source::Timer));
        assert_eq!(
            source(&automation, "2026-11-02T09:30"),
            Some(Source::Schedule)
        );
        assert_eq!(
            automation.next_change(at("2026-11-02T08:45"), &tz),
            Some(NextChange {
                at: at("2026-11-02T18:00"),
                on: false,
                source: None,
            })
        );
    }

    #[test]
    fn a_timer_outlasts_the_window_it_started_in() {
        let tz = paris();
        let mut automation = automation();
        automation.start_timer(at("2026-11-02T19:00"));
        assert_eq!(source(&automation, "2026-11-02T18:30"), Some(Source::Timer));
        assert_eq!(
            automation.next_change(at("2026-11-02T17:00"), &tz),
            Some(NextChange {
                at: at("2026-11-02T19:00"),
                on: false,
                source: None,
            })
        );
    }

    #[test]
    fn manual_cancels_a_timer_and_a_timer_replaces_manual() {
        let tz = paris();
        let mut automation = Automation::default();
        automation.start_timer(at("2026-11-02T20:00"));
        automation.set_manual(true, at("2026-11-02T19:00"), &tz);
        assert_eq!(automation.timer_until(at("2026-11-02T19:00")), None);
        assert_eq!(
            source(&automation, "2026-11-02T21:00"),
            Some(Source::Manual)
        );

        automation.start_timer(at("2026-11-02T22:00"));
        assert_eq!(source(&automation, "2026-11-02T21:00"), Some(Source::Timer));
        assert_eq!(source(&automation, "2026-11-02T22:00"), None);
    }

    #[test]
    fn expired_overrides_are_forgotten() {
        let mut automation = Automation::default();
        automation.start_timer(at("2026-11-02T20:00"));
        assert!(automation.is_watching());
        automation.expire(at("2026-11-02T19:59"));
        assert!(automation.is_watching());
        automation.expire(at("2026-11-02T20:00"));
        assert!(!automation.is_watching());
    }

    #[test]
    fn a_new_schedule_re_anchors_a_manual_override() {
        let tz = paris();
        let mut automation = Automation::default();
        automation.set_manual(true, at("2026-11-02T07:00"), &tz);
        automation.set_schedule(office_hours(), at("2026-11-02T08:00"), &tz);
        assert_eq!(
            source(&automation, "2026-11-02T08:30"),
            Some(Source::Manual)
        );
        assert_eq!(
            source(&automation, "2026-11-02T12:00"),
            Some(Source::Schedule)
        );
        assert_eq!(source(&automation, "2026-11-02T18:00"), None);
    }

    fn meetings() -> Vec<(Timestamp, Timestamp)> {
        vec![
            (at("2026-11-02T19:00"), at("2026-11-02T19:30")),
            // Overlaps the window's end: ON straight through.
            (at("2026-11-03T17:30"), at("2026-11-03T18:30")),
        ]
    }

    #[test]
    fn meetings_turn_it_on_like_windows() {
        let tz = paris();
        let mut automation = automation();
        automation.set_meetings(meetings(), at("2026-11-02T00:00"), &tz);
        assert!(automation.is_watching());
        assert_eq!(source(&automation, "2026-11-02T18:59"), None);
        assert_eq!(
            source(&automation, "2026-11-02T19:00"),
            Some(Source::Calendar)
        );
        assert_eq!(source(&automation, "2026-11-02T19:30"), None);
        assert_eq!(
            automation.next_change(at("2026-11-02T18:00"), &tz),
            Some(NextChange {
                at: at("2026-11-02T19:00"),
                on: true,
                source: Some(Source::Calendar),
            })
        );
        // The window wins the source; the meeting carries it past 18:00.
        assert_eq!(
            source(&automation, "2026-11-03T17:45"),
            Some(Source::Schedule)
        );
        assert_eq!(
            source(&automation, "2026-11-03T18:15"),
            Some(Source::Calendar)
        );
        assert_eq!(
            automation.next_change(at("2026-11-03T12:00"), &tz),
            Some(NextChange {
                at: at("2026-11-03T18:30"),
                on: false,
                source: None,
            })
        );

        // Calendar only, no schedule.
        let mut automation = Automation::default();
        automation.set_meetings(meetings(), at("2026-11-02T00:00"), &tz);
        assert_eq!(
            source(&automation, "2026-11-02T19:10"),
            Some(Source::Calendar)
        );
    }

    #[test]
    fn manual_off_pauses_a_meeting_until_it_ends() {
        let tz = paris();
        let mut automation = Automation::default();
        automation.set_meetings(meetings(), at("2026-11-02T00:00"), &tz);
        automation.set_manual(false, at("2026-11-02T19:05"), &tz);
        assert_eq!(source(&automation, "2026-11-02T19:10"), None);
        assert_eq!(
            automation.paused(at("2026-11-02T19:10"), &tz),
            Some((at("2026-11-02T19:30"), Source::Calendar))
        );
        assert_eq!(
            source(&automation, "2026-11-03T17:30"),
            Some(Source::Calendar)
        );
    }

    #[test]
    fn new_meetings_re_anchor_a_manual_override() {
        let tz = paris();
        let mut automation = Automation::default();
        automation.set_manual(true, at("2026-11-02T18:00"), &tz);
        automation.set_meetings(meetings(), at("2026-11-02T18:30"), &tz);
        // ON by hand until the meeting takes over, then OFF when it ends.
        assert_eq!(
            source(&automation, "2026-11-02T18:45"),
            Some(Source::Manual)
        );
        assert_eq!(
            source(&automation, "2026-11-02T19:15"),
            Some(Source::Calendar)
        );
        assert_eq!(source(&automation, "2026-11-02T19:30"), None);
        automation.expire(at("2026-11-02T19:40"));
        automation.set_meetings(Vec::new(), at("2026-11-02T19:40"), &tz);
        assert!(!automation.is_watching());
    }

    #[test]
    fn a_sleep_past_the_end_lands_in_the_right_state() {
        // Nothing is stepped through: the state after waking up is computed
        // from the wall clock alone.
        let tz = paris();
        let mut automation = automation();
        automation.start_timer(at("2026-11-02T20:00"));
        let woke = at("2026-11-03T10:00");
        automation.expire(woke);
        assert_eq!(automation.decide(woke, &tz).source, Some(Source::Schedule));
        assert_eq!(automation.timer_until(woke), None);
    }
}

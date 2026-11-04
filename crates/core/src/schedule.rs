//! Weekly schedules (ROADMAP Phase 2): time windows in local wall-clock time,
//! per weekday, with skip days. No OS calls here, and no state: everything is
//! computed from the time it is asked about, so a sleep, a resume or a DST
//! change can never leave a stale answer behind.

use jiff::civil::{Date, Weekday};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan as _};

pub const MINUTES_PER_DAY: u16 = 24 * 60;
/// How far ahead the next change is looked for. Two weeks covers a run of
/// skip days around a holiday.
const LOOKAHEAD_DAYS: i64 = 15;

/// A set of weekdays, Monday first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Weekdays(u8);

impl Weekdays {
    pub const NAMES: [&'static str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    pub const WORKDAYS: Self = Self(0b001_1111);
    pub const EVERY_DAY: Self = Self(0b111_1111);

    pub fn empty() -> Self {
        Self(0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn contains(self, day: Weekday) -> bool {
        self.0 & (1 << day.to_monday_zero_offset()) != 0
    }

    pub fn with(self, day: Weekday) -> Self {
        Self(self.0 | 1 << day.to_monday_zero_offset())
    }

    /// `"mon"` to `"sun"`.
    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        names.into_iter().try_fold(Self::empty(), |days, name| {
            let index = Self::NAMES.iter().position(|day| *day == name)?;
            Some(Self(days.0 | 1 << index))
        })
    }

    /// Monday first.
    pub fn names(self) -> Vec<&'static str> {
        (0..7)
            .filter(|index| self.0 & (1 << index) != 0)
            .map(|index| Self::NAMES[index])
            .collect()
    }
}

/// "ON from `start` to `end` on these days". Times are minutes since local
/// midnight. An `end` at or before `start` ends the next day, so 22:00 to
/// 06:00 runs overnight. An `end` of 1440 is midnight at the end of the day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimeWindow {
    pub days: Weekdays,
    pub start: u16,
    pub end: u16,
}

impl TimeWindow {
    /// `None` unless at least one day is set, `start` is 0 to 1439 and `end`
    /// is 1 to 1440.
    pub fn new(days: Weekdays, start: u16, end: u16) -> Option<Self> {
        (!days.is_empty() && start < MINUTES_PER_DAY && (1..=MINUTES_PER_DAY).contains(&end))
            .then_some(Self { days, start, end })
    }

    /// The window that starts on `date`, if it runs that day.
    fn interval_on(&self, date: Date, tz: &TimeZone) -> Option<(Timestamp, Timestamp)> {
        if !self.days.contains(date.weekday()) {
            return None;
        }
        let end_date = if self.end <= self.start {
            date.tomorrow().ok()?
        } else {
            date
        };
        let start = local(date, self.start, tz)?;
        let end = local(end_date, self.end, tz)?;
        (start < end).then_some((start, end))
    }
}

/// `"09:30"` to minutes. `"24:00"` is accepted as the end of the day.
pub fn parse_time(text: &str) -> Option<u16> {
    let (hours, minutes) = text.split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    let hours: u16 = hours.parse().ok()?;
    let minutes: u16 = minutes.parse().ok()?;
    let total = hours * 60 + minutes;
    (minutes < 60 && total <= MINUTES_PER_DAY).then_some(total)
}

/// Minutes to `"09:30"`.
pub fn format_time(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// A wall-clock time on a date, in `tz`. A time that a DST jump skips
/// resolves to the same offset as before the jump (so 02:30 on a spring-
/// forward night is 03:30), and a time that happens twice resolves to the
/// first one, as most calendars do.
fn local(date: Date, minutes: u16, tz: &TimeZone) -> Option<Timestamp> {
    let (date, minutes) = if minutes >= MINUTES_PER_DAY {
        (date.tomorrow().ok()?, minutes - MINUTES_PER_DAY)
    } else {
        (date, minutes)
    };
    let time = date.at((minutes / 60) as i8, (minutes % 60) as i8, 0, 0);
    tz.to_ambiguous_zoned(time)
        .compatible()
        .ok()
        .map(|zoned| zoned.timestamp())
}

/// The next time the clock shows `minutes` after `now`: today, or tomorrow if
/// that time has passed. For "until 18:00" timers (SPEC SC-3).
pub fn next_local_time(now: &jiff::Zoned, minutes: u16) -> Option<Timestamp> {
    let tz = now.time_zone();
    let today = local(now.date(), minutes, tz)?;
    if today > now.timestamp() {
        return Some(today);
    }
    local(now.date().tomorrow().ok()?, minutes, tz)
}

/// When the app turns itself on, every week.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schedule {
    pub enabled: bool,
    pub windows: Vec<TimeWindow>,
    /// Days off: a window that starts on one of these dates doesn't run.
    pub skip_dates: Vec<Date>,
}

impl Schedule {
    /// Whether the schedule can ever turn anything on.
    pub fn is_active(&self) -> bool {
        self.enabled && !self.windows.is_empty()
    }

    /// Whether `at` falls inside a window.
    pub fn is_on(&self, at: Timestamp, tz: &TimeZone) -> bool {
        if !self.is_active() {
            return false;
        }
        let today = at.to_zoned(tz.clone()).date();
        // Yesterday's overnight windows can still be running.
        self.intervals(today.yesterday().unwrap_or(today), today, tz)
            .any(|(start, end)| start <= at && at < end)
    }

    /// The first time after `at` when [`Self::is_on`] changes, if it does
    /// within the next two weeks.
    pub fn next_flip(&self, at: Timestamp, tz: &TimeZone) -> Option<Timestamp> {
        let on = self.is_on(at, tz);
        self.boundaries_after(at, tz)
            .into_iter()
            .find(|&boundary| self.is_on(boundary, tz) != on)
    }

    /// Every window start and end after `at`, in order, without duplicates.
    pub fn boundaries_after(&self, at: Timestamp, tz: &TimeZone) -> Vec<Timestamp> {
        if !self.is_active() {
            return Vec::new();
        }
        let today = at.to_zoned(tz.clone()).date();
        let from = today.yesterday().unwrap_or(today);
        let to = today.checked_add(LOOKAHEAD_DAYS.days()).unwrap_or(today);
        let mut boundaries: Vec<Timestamp> = self
            .intervals(from, to, tz)
            .flat_map(|(start, end)| [start, end])
            .filter(|&boundary| boundary > at)
            .collect();
        boundaries.sort_unstable();
        boundaries.dedup();
        boundaries
    }

    /// The windows that start between `from` and `to`, both included.
    fn intervals<'a>(
        &'a self,
        from: Date,
        to: Date,
        tz: &'a TimeZone,
    ) -> impl Iterator<Item = (Timestamp, Timestamp)> + 'a {
        from.series(1.day())
            .take_while(move |date| *date <= to)
            .filter(|date| !self.skip_dates.contains(date))
            .flat_map(move |date| {
                self.windows
                    .iter()
                    .filter_map(move |window| window.interval_on(date, tz))
            })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use jiff::civil::date;

    use super::*;

    /// Central European time, with its DST rules, without needing a time zone
    /// database on the test machine.
    pub(crate) fn paris() -> TimeZone {
        TimeZone::posix("CET-1CEST,M3.5.0,M10.5.0/3").unwrap()
    }

    pub(crate) fn at(text: &str) -> Timestamp {
        let time: jiff::civil::DateTime = text.parse().unwrap();
        paris()
            .to_ambiguous_zoned(time)
            .compatible()
            .unwrap()
            .timestamp()
    }

    pub(crate) fn office_hours() -> Schedule {
        Schedule {
            enabled: true,
            windows: vec![TimeWindow::new(Weekdays::WORKDAYS, 9 * 60, 18 * 60).unwrap()],
            skip_dates: Vec::new(),
        }
    }

    #[test]
    fn times_parse_and_format() {
        assert_eq!(parse_time("09:30"), Some(570));
        assert_eq!(parse_time("00:00"), Some(0));
        assert_eq!(parse_time("24:00"), Some(1440));
        for bad in ["24:01", "9:30", "09:60", "0930", "", "ab:cd"] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
        assert_eq!(format_time(570), "09:30");
        assert_eq!(format_time(1440), "24:00");
    }

    #[test]
    fn until_a_time_means_the_next_time_the_clock_shows_it() {
        let now = at("2026-11-02T17:00").to_zoned(paris());
        assert_eq!(next_local_time(&now, 18 * 60), Some(at("2026-11-02T18:00")));
        assert_eq!(next_local_time(&now, 9 * 60), Some(at("2026-11-03T09:00")));
        assert_eq!(next_local_time(&now, 17 * 60), Some(at("2026-11-03T17:00")));
    }

    #[test]
    fn weekday_names_round_trip() {
        let days = Weekdays::from_names(["mon", "wed", "sun"]).unwrap();
        assert_eq!(days.names(), ["mon", "wed", "sun"]);
        assert!(days.contains(Weekday::Sunday));
        assert!(!days.contains(Weekday::Tuesday));
        assert_eq!(Weekdays::from_names(["funday"]), None);
        assert_eq!(Weekdays::WORKDAYS.names().len(), 5);
    }

    #[test]
    fn windows_are_validated() {
        assert!(TimeWindow::new(Weekdays::empty(), 0, 60).is_none());
        assert!(TimeWindow::new(Weekdays::WORKDAYS, 1440, 60).is_none());
        assert!(TimeWindow::new(Weekdays::WORKDAYS, 0, 0).is_none());
        assert!(TimeWindow::new(Weekdays::WORKDAYS, 0, 1440).is_some());
    }

    #[test]
    fn on_inside_a_window_only() {
        let schedule = office_hours();
        let tz = paris();
        // 2026-11-02 is a Monday.
        assert!(!schedule.is_on(at("2026-11-02T08:59:59"), &tz));
        assert!(schedule.is_on(at("2026-11-02T09:00"), &tz));
        assert!(schedule.is_on(at("2026-11-02T17:59:59"), &tz));
        assert!(!schedule.is_on(at("2026-11-02T18:00"), &tz));
        assert!(!schedule.is_on(at("2026-11-07T10:00"), &tz), "Saturday");
    }

    #[test]
    fn disabled_or_empty_is_never_on() {
        let tz = paris();
        let disabled = Schedule {
            enabled: false,
            ..office_hours()
        };
        assert!(!disabled.is_on(at("2026-11-02T10:00"), &tz));
        assert_eq!(disabled.next_flip(at("2026-11-02T10:00"), &tz), None);
        let empty = Schedule {
            enabled: true,
            ..Schedule::default()
        };
        assert!(!empty.is_active());
    }

    #[test]
    fn overnight_windows_run_into_the_next_day() {
        let tz = paris();
        let schedule = Schedule {
            enabled: true,
            windows: vec![
                TimeWindow::new(Weekdays::from_names(["fri"]).unwrap(), 22 * 60, 6 * 60).unwrap(),
            ],
            skip_dates: Vec::new(),
        };
        assert!(schedule.is_on(at("2026-11-06T23:00"), &tz));
        assert!(
            schedule.is_on(at("2026-11-07T05:59"), &tz),
            "Saturday morning"
        );
        assert!(!schedule.is_on(at("2026-11-07T06:00"), &tz));
        assert!(
            !schedule.is_on(at("2026-11-07T23:00"), &tz),
            "Saturday night"
        );
    }

    #[test]
    fn skip_days_skip_the_windows_that_start_on_them() {
        let tz = paris();
        let schedule = Schedule {
            skip_dates: vec![date(2026, 11, 11)],
            ..office_hours()
        };
        assert!(schedule.is_on(at("2026-11-10T10:00"), &tz));
        assert!(!schedule.is_on(at("2026-11-11T10:00"), &tz));
        // The next flip from the evening before is the day after.
        assert_eq!(
            schedule.next_flip(at("2026-11-10T19:00"), &tz),
            Some(at("2026-11-12T09:00"))
        );
    }

    #[test]
    fn overlapping_windows_merge() {
        let tz = paris();
        let schedule = Schedule {
            enabled: true,
            windows: vec![
                TimeWindow::new(Weekdays::EVERY_DAY, 9 * 60, 12 * 60).unwrap(),
                TimeWindow::new(Weekdays::EVERY_DAY, 11 * 60, 14 * 60).unwrap(),
            ],
            skip_dates: Vec::new(),
        };
        assert_eq!(
            schedule.next_flip(at("2026-11-02T10:00"), &tz),
            Some(at("2026-11-02T14:00"))
        );
    }

    #[test]
    fn next_flip_skips_the_weekend() {
        let tz = paris();
        assert_eq!(
            office_hours().next_flip(at("2026-11-06T18:30"), &tz),
            Some(at("2026-11-09T09:00"))
        );
    }

    #[test]
    fn wall_clock_times_hold_across_dst() {
        let tz = paris();
        let schedule = Schedule {
            enabled: true,
            windows: vec![TimeWindow::new(Weekdays::EVERY_DAY, 9 * 60, 18 * 60).unwrap()],
            skip_dates: Vec::new(),
        };
        // Clocks go back at 03:00 on 2026-10-25 and forward at 02:00 on
        // 2027-03-28. 09:00 stays 09:00 on both sides.
        assert_eq!(
            schedule.next_flip(at("2026-10-24T20:00"), &tz),
            Some(at("2026-10-25T09:00"))
        );
        assert_eq!(
            schedule.next_flip(at("2027-03-27T20:00"), &tz),
            Some(at("2027-03-28T09:00"))
        );
        // 24 hours of real time apart, but 09:00 to 09:00 across the change
        // is 25 hours.
        assert_eq!(
            at("2026-10-25T09:00").duration_since(at("2026-10-24T09:00")),
            jiff::SignedDuration::from_hours(25)
        );
    }

    #[test]
    fn a_window_starting_in_the_dst_gap_starts_after_it() {
        let tz = paris();
        let schedule = Schedule {
            enabled: true,
            windows: vec![TimeWindow::new(Weekdays::EVERY_DAY, 2 * 60 + 30, 4 * 60).unwrap()],
            skip_dates: Vec::new(),
        };
        // 02:30 doesn't exist on 2027-03-28; the window opens at 03:30.
        let start = schedule.next_flip(at("2027-03-28T00:00"), &tz).unwrap();
        assert_eq!(start.to_zoned(tz.clone()).time().hour(), 3);
        assert!(schedule.is_on(at("2027-03-28T03:45"), &tz));
    }
}

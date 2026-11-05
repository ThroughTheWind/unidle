//! Text the Rust side shows: the tray, its menu, notifications and the macOS
//! app menu. The window's text is in `ui/src/strings.ts` (SPEC NFR-5).

use std::time::Duration;

use jiff::{Timestamp, ToSpan as _, Zoned};
use unidle_core::{Hold, Source, Status};

#[cfg(target_os = "macos")]
pub const APP_NAME: &str = "Unidle";
pub const TOOLTIP_ON: &str = "Unidle: ON";
pub const TOOLTIP_OFF: &str = "Unidle: OFF";
pub const TURN_ON: &str = "Turn ON";
pub const TURN_OFF: &str = "Turn OFF";
pub const OPEN_WINDOW: &str = "Open window";
pub const QUIT: &str = "Quit";
#[cfg(target_os = "macos")]
pub const QUIT_APP: &str = "Quit Unidle";
pub const STATUS_OFF: &str = "Activity: OFF";
pub const STILL_RUNNING: &str = "Still running in the tray";
pub const STILL_RUNNING_BODY: &str =
    "Unidle keeps working with its window closed. Quit it from the tray menu.";

pub const KEEP_AWAKE_FOR: &str = "Keep awake for";
/// The quick timers (SPEC SC-3), in minutes, with their menu labels.
pub const TIMERS: [(u32, &str); 4] = [
    (15, "15 minutes"),
    (30, "30 minutes"),
    (60, "1 hour"),
    (120, "2 hours"),
];
pub const TIMER_UNTIL: &str = "Until a time…";
pub const SCHEDULE_FILE_NAME: &str = "unidle-schedule.json";
pub const SCHEDULE_FILE_FILTER: &str = "Unidle schedule";
pub const ACTIVITY_FILE_NAME: &str = "unidle-activity.csv";
pub const ACTIVITY_FILE_FILTER: &str = "CSV";

/// Wanted ON, held by a condition (SPEC §14).
pub fn held(hold: Hold) -> &'static str {
    match hold {
        Hold::Locked => "Paused while the screen is locked",
        Hold::OnBattery => "Paused on battery power",
        Hold::LowBattery => "Paused: battery low",
        Hold::AppNotRunning => "Paused until a chosen app runs",
    }
}

/// The tray's first line: ON for how long, paused and why, or OFF.
pub fn status_line(status: &Status) -> String {
    if let Some(hold) = status.held {
        held(hold).to_string()
    } else if status.is_on() {
        status_on(status.elapsed)
    } else {
        STATUS_OFF.to_string()
    }
}

pub const CLI_NOT_RUNNING: &str = "Activity: OFF (Unidle isn't running)";
pub const CLI_START_FAILED: &str = "Unidle didn't start, or an older version is running that has no command line. Quit it and try again.";
pub const CLI_HOW_TO_STOP: &str = "Stop it with `unidle off`, or from the tray.";
pub const CLI_POWER_ONLY: &str = "Power only: no input nudges";
pub const CLI_NUDGING: &str = "Input nudges: on";
pub const CLI_NOT_NUDGING: &str = "Input nudges: off";

/// The status for `unidle status` (SPEC §13): the tray's lines, then
/// whether nudges are sent.
pub fn cli_status(status: &Status, now: &Zoned) -> String {
    let mut lines = vec![status_line(status)];
    lines.extend(detail(status, now));
    if status.is_on() {
        lines.push(
            if status.power_only {
                CLI_POWER_ONLY
            } else if status.nudging {
                CLI_NUDGING
            } else {
                CLI_NOT_NUDGING
            }
            .to_string(),
        );
    }
    lines.join("\n")
}

/// "Activity: ON for 1 h 12 min" (SPEC FR-5).
pub fn status_on(elapsed: Duration) -> String {
    let minutes = elapsed.as_secs() / 60;
    match (minutes / 60, minutes % 60) {
        (0, 0) => "Activity: ON for less than a minute".to_string(),
        (0, m) => format!("Activity: ON for {m} min"),
        (h, 0) => format!("Activity: ON for {h} h"),
        (h, m) => format!("Activity: ON for {h} h {m} min"),
    }
}

/// The source and the next change (SPEC SC-4), for example "Timer · OFF at
/// 18:00" or "Scheduled · ON tomorrow at 09:00". `None` when there is
/// nothing to say: OFF with nothing planned.
pub fn detail(status: &Status, now: &Zoned) -> Option<String> {
    let next = status
        .next_change
        .map(|change| when(if change.on { "ON" } else { "OFF" }, change.at, now));
    if let Some(until) = status.paused_until {
        return Some(format!("Schedule paused until {}", clock(until, now)));
    }
    let source = match status.source {
        Some(Source::Manual) => "Manual",
        Some(Source::Timer) => "Timer",
        Some(Source::Schedule) => "Scheduled",
        // OFF: only a planned ON is worth a line, and only the schedule plans
        // one.
        None => return next.map(|next| format!("Scheduled · {next}")),
    };
    Some(match next {
        Some(next) => format!("{source} · {next}"),
        None => source.to_string(),
    })
}

/// "OFF at 18:00", "ON tomorrow at 09:00", "ON Mon at 09:00",
/// "ON Nov 16 at 09:00".
fn when(state: &str, at: Timestamp, now: &Zoned) -> String {
    let local = at.to_zoned(now.time_zone().clone());
    let days = (local.date() - now.date()).get_days();
    let time = local.strftime("%H:%M");
    match days {
        ..=0 => format!("{state} at {time}"),
        1 => format!("{state} tomorrow at {time}"),
        2..=6 => format!("{state} {} at {time}", local.strftime("%a")),
        _ => format!("{state} {} at {time}", local.strftime("%b %-d")),
    }
}

/// "18:00", or "tomorrow 18:00" when it isn't today.
fn clock(at: Timestamp, now: &Zoned) -> String {
    let local = at.to_zoned(now.time_zone().clone());
    let time = local.strftime("%H:%M").to_string();
    if local.date() == now.date() {
        time
    } else if now.date().checked_add(1.day()).ok() == Some(local.date()) {
        format!("tomorrow {time}")
    } else {
        format!("{} {time}", local.strftime("%a"))
    }
}

#[cfg(test)]
mod tests {
    use jiff::tz::TimeZone;
    use unidle_core::{Capabilities, NextChange};

    use super::*;

    fn zoned(text: &str) -> Zoned {
        let time: jiff::civil::DateTime = text.parse().unwrap();
        time.to_zoned(TimeZone::UTC).unwrap()
    }

    fn status(source: Option<Source>, next: Option<(&str, bool)>) -> Status {
        Status {
            source,
            elapsed: Duration::ZERO,
            capabilities: Capabilities {
                power_assertion: true,
                input_nudge: true,
                input_unavailable: None,
            },
            nudging: false,
            next_change: next.map(|(at, on)| NextChange {
                at: zoned(at).timestamp(),
                on,
            }),
            paused_until: None,
            power_only: false,
            held: None,
        }
    }

    #[test]
    fn detail_names_the_source_and_the_next_change() {
        // 2026-11-02 is a Monday.
        let now = zoned("2026-11-02T17:00");
        let detail = |status: Status| detail(&status, &now);
        assert_eq!(detail(status(None, None)), None);
        assert_eq!(
            detail(status(Some(Source::Manual), None)).as_deref(),
            Some("Manual")
        );
        assert_eq!(
            detail(status(
                Some(Source::Timer),
                Some(("2026-11-02T18:00", false))
            ))
            .as_deref(),
            Some("Timer · OFF at 18:00")
        );
        assert_eq!(
            detail(status(None, Some(("2026-11-03T09:00", true)))).as_deref(),
            Some("Scheduled · ON tomorrow at 09:00")
        );
        assert_eq!(
            detail(status(
                Some(Source::Schedule),
                Some(("2026-11-06T18:00", false))
            ))
            .as_deref(),
            Some("Scheduled · OFF Fri at 18:00")
        );
        assert_eq!(
            detail(status(None, Some(("2026-11-16T09:00", true)))).as_deref(),
            Some("Scheduled · ON Nov 16 at 09:00")
        );
        let paused = Status {
            paused_until: Some(zoned("2026-11-02T18:00").timestamp()),
            ..status(None, Some(("2026-11-03T09:00", true)))
        };
        assert_eq!(
            detail(paused).as_deref(),
            Some("Schedule paused until 18:00")
        );
    }

    #[test]
    fn cli_status_adds_the_nudges_line_while_on() {
        let now = zoned("2026-11-02T17:00");
        assert_eq!(cli_status(&status(None, None), &now), "Activity: OFF");
        let on = Status {
            power_only: true,
            ..status(Some(Source::Timer), Some(("2026-11-02T18:00", false)))
        };
        assert_eq!(
            cli_status(&on, &now),
            "Activity: ON for less than a minute\nTimer · OFF at 18:00\nPower only: no input nudges"
        );
    }

    #[test]
    fn elapsed_reads_like_the_spec() {
        let at = |secs| status_on(Duration::from_secs(secs));
        assert_eq!(at(20), "Activity: ON for less than a minute");
        assert_eq!(at(12 * 60 + 59), "Activity: ON for 12 min");
        assert_eq!(at(3600), "Activity: ON for 1 h");
        assert_eq!(at(72 * 60), "Activity: ON for 1 h 12 min");
    }
}

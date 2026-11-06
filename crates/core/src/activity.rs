//! The activity log (SPEC §15, ROADMAP Phase 4): one CSV row per ON session,
//! kept only when the user turns it on, and only on this computer.
//!
//! A row says when a session started and ended, why it was ON, whether it was
//! power-only and how many nudges it sent. Nothing about what the user did or
//! typed is ever written.

use std::io;
use std::path::Path;

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::controller::Source;

/// The first line of the file.
pub const CSV_HEADER: &str = "start,end,duration_seconds,source,power_only,nudges";

/// The most rows the file keeps. The oldest go first, so a year of sessions
/// stays a small file.
pub const MAX_ROWS: usize = 5000;

/// One ON session, from turning ON to turning OFF or being paused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Session {
    pub start: Timestamp,
    pub end: Timestamp,
    /// Why it turned ON. A timer that ends inside a schedule window keeps
    /// the session going, under the source it started with.
    pub source: Source,
    /// Power-only for some or all of it, as `unidle on` asks (SPEC §13).
    pub power_only: bool,
    pub nudges: u32,
}

impl Session {
    /// A CSV row in local time with the UTC offset, so it sorts and reads in
    /// any spreadsheet.
    pub fn csv_row(&self, tz: &TimeZone) -> String {
        let time = |at: Timestamp| at.to_zoned(tz.clone()).strftime("%Y-%m-%dT%H:%M:%S%:z");
        let seconds = self.end.duration_since(self.start).as_secs().max(0);
        format!(
            "{},{},{},{},{},{}",
            time(self.start),
            time(self.end),
            seconds,
            source_name(self.source),
            self.power_only,
            self.nudges,
        )
    }
}

pub const fn source_name(source: Source) -> &'static str {
    match source {
        Source::Manual => "manual",
        Source::Timer => "timer",
        Source::Schedule => "schedule",
        Source::Calendar => "calendar",
    }
}

/// `existing` with `row` added, the header first and at most [`MAX_ROWS`]
/// rows. Lines that aren't rows, such as a header from an older version, are
/// dropped.
pub fn append_row(existing: &str, row: &str) -> String {
    let mut rows: Vec<&str> = existing
        .lines()
        .filter(|line| !line.is_empty() && *line != CSV_HEADER && !line.starts_with("start,"))
        .collect();
    rows.push(row);
    let skip = rows.len().saturating_sub(MAX_ROWS);
    let mut text = String::with_capacity(existing.len() + row.len() + CSV_HEADER.len() + 2);
    text.push_str(CSV_HEADER);
    text.push('\n');
    for row in &rows[skip..] {
        text.push_str(row);
        text.push('\n');
    }
    text
}

/// Adds `session` to the log at `path`, creating it if needed. Written to a
/// temporary file and renamed, so a crash never leaves half a file.
pub fn append(path: &Path, session: &Session, tz: &TimeZone) -> io::Result<()> {
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temporary = path.with_extension("csv.tmp");
    std::fs::write(&temporary, append_row(&existing, &session.csv_row(tz)))?;
    std::fs::rename(&temporary, path)
}

/// The log's text for an export: the header alone when nothing was logged.
pub fn read(path: &Path) -> io::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => Ok(text),
        Ok(_) => Ok(format!("{CSV_HEADER}\n")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(format!("{CSV_HEADER}\n")),
        Err(error) => Err(error),
    }
}

/// Deletes the log. A log that isn't there is already clear.
pub fn clear(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            start: "2026-11-05T08:00:00Z".parse().unwrap(),
            end: "2026-11-05T09:30:05Z".parse().unwrap(),
            source: Source::Schedule,
            power_only: false,
            nudges: 42,
        }
    }

    #[test]
    fn a_row_is_local_time_with_its_offset() {
        let tz = TimeZone::get("Europe/Paris").unwrap();
        assert_eq!(
            session().csv_row(&tz),
            "2026-11-05T09:00:00+01:00,2026-11-05T10:30:05+01:00,5405,schedule,false,42"
        );
    }

    #[test]
    fn appending_keeps_one_header_and_the_latest_rows() {
        let text = append_row("", "a");
        assert_eq!(text, format!("{CSV_HEADER}\na\n"));
        let text = append_row(&text, "b");
        assert_eq!(text, format!("{CSV_HEADER}\na\nb\n"));

        let mut text = String::new();
        for row in 0..MAX_ROWS + 3 {
            text = append_row(&text, &row.to_string());
        }
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), MAX_ROWS + 1);
        assert_eq!(lines[0], CSV_HEADER);
        assert_eq!(lines[1], "3");
        assert_eq!(lines[MAX_ROWS], (MAX_ROWS + 2).to_string());
    }

    #[test]
    fn the_file_is_created_read_and_cleared() {
        let dir = std::env::temp_dir().join(format!("unidle-activity-{}", std::process::id()));
        let path = dir.join("activity-log.csv");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(read(&path).unwrap(), format!("{CSV_HEADER}\n"));

        append(&path, &session(), &TimeZone::UTC).unwrap();
        append(&path, &session(), &TimeZone::UTC).unwrap();
        let text = read(&path).unwrap();
        assert_eq!(text.lines().count(), 3);
        assert!(
            text.lines()
                .nth(1)
                .unwrap()
                .starts_with("2026-11-05T08:00:00+00:00,")
        );

        clear(&path).unwrap();
        clear(&path).unwrap();
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

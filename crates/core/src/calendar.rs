//! Calendar awareness (SPEC §18): the busy times in a local iCalendar
//! (`.ics`) file, so the app can turn ON during meetings. The file is only
//! read, never fetched (NFR-2).
//!
//! Only what decides when a meeting runs is read: the start, the end or
//! duration, the recurrence rule with its exceptions, and whether the event
//! is cancelled or marked free. Titles, places and attendees are skipped and
//! never kept. Anything this reader doesn't understand turns nothing ON: an
//! all-day event, a rule it can't follow past its first meeting, a date it
//! can't read.

use jiff::civil::{Date, DateTime, Time, Weekday};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, ToSpan as _};

/// A file bigger than this is not a calendar the app should read every few
/// minutes.
pub const MAX_FILE_BYTES: u64 = 20_000_000;
/// Enough for any real calendar, small enough that a bad file stays cheap.
const MAX_EVENTS: usize = 20_000;
/// Occurrences one rule may produce while looking for the range, so a daily
/// rule from 1990 stays cheap.
const MAX_STEPS: usize = 20_000;

/// A start or end as the file wrote it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum When {
    /// `Z`: an absolute time.
    Utc(Timestamp),
    /// With a `TZID` the app knows, or floating (the computer's own zone)
    /// when `zone` is `None`.
    Local(DateTime, Option<Zone>),
}

/// An index into [`Calendar::zones`], so events share one `TimeZone`.
type Zone = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Rule {
    frequency: Frequency,
    interval: i32,
    count: Option<usize>,
    until: Option<When>,
    /// With an ordinal for monthly rules: `-1FR` is the last Friday.
    by_day: Vec<(Option<i8>, Weekday)>,
    by_month_day: Vec<i8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Event {
    uid: Option<String>,
    start: When,
    length: SignedDuration,
    rule: Option<Rule>,
    /// Occurrences left out, by their start.
    except: Vec<When>,
    /// This event replaces the occurrence of `uid` that started then.
    replaces: Option<When>,
}

/// The meetings in a calendar file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Calendar {
    events: Vec<Event>,
    zones: Vec<TimeZone>,
}

impl Calendar {
    /// Reads iCalendar text. Never fails: an event it can't read is left out,
    /// and a file with none gives an empty calendar.
    pub fn parse(text: &str) -> Self {
        let mut calendar = Self::default();
        let mut zone_names: Vec<String> = Vec::new();
        let mut current: Option<Draft> = None;
        // Components inside an event, such as an alarm, are skipped.
        let mut depth_inside_event = 0usize;
        for line in unfold(text) {
            let Some(property) = Property::parse(&line) else {
                continue;
            };
            match (property.name.as_str(), &mut current) {
                ("BEGIN", None) if property.value.eq_ignore_ascii_case("VEVENT") => {
                    current = Some(Draft::default());
                    depth_inside_event = 0;
                }
                ("BEGIN", Some(_)) => depth_inside_event += 1,
                ("END", Some(_)) if depth_inside_event > 0 => depth_inside_event -= 1,
                ("END", Some(_)) => {
                    let draft = current.take().unwrap_or_default();
                    if calendar.events.len() < MAX_EVENTS
                        && let Some(event) = draft.finish(&mut calendar.zones, &mut zone_names)
                    {
                        calendar.events.push(event);
                    }
                }
                (_, Some(draft)) if depth_inside_event == 0 => draft.add(property),
                _ => {}
            }
        }
        calendar
    }

    /// How many events the file has that can turn the app ON, counting a
    /// recurring meeting once.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The busy times that overlap `from..to`, sorted and merged, with
    /// floating times read in `tz`.
    pub fn busy(
        &self,
        from: Timestamp,
        to: Timestamp,
        tz: &TimeZone,
    ) -> Vec<(Timestamp, Timestamp)> {
        let mut busy = Vec::new();
        for event in self.events.iter().filter(|event| event.replaces.is_none()) {
            let moved: Vec<Timestamp> = self
                .events
                .iter()
                .filter(|other| other.uid.is_some() && other.uid == event.uid)
                .filter_map(|other| other.replaces)
                .filter_map(|when| self.instant(when, tz))
                .collect();
            let except: Vec<Timestamp> = event
                .except
                .iter()
                .filter_map(|&when| self.instant(when, tz))
                .chain(moved)
                .collect();
            for start in self.occurrences(event, from, to, tz) {
                let Ok(end) = start.checked_add(event.length) else {
                    continue;
                };
                if end > from && !except.contains(&start) {
                    busy.push((start, end));
                }
            }
        }
        // Moved occurrences, cancelled ones excepted (they were never added).
        for event in self.events.iter().filter(|event| event.replaces.is_some()) {
            if let Some(start) = self.instant(event.start, tz)
                && let Ok(end) = start.checked_add(event.length)
                && start < to
                && end > from
            {
                busy.push((start, end));
            }
        }
        merge(busy)
    }

    /// The starts of `event` before `to`, in order. A rule without a count
    /// starts looking shortly before `from`, so an old rule stays cheap.
    fn occurrences(
        &self,
        event: &Event,
        from: Timestamp,
        to: Timestamp,
        tz: &TimeZone,
    ) -> Vec<Timestamp> {
        let Some(first) = self.instant(event.start, tz) else {
            return Vec::new();
        };
        let Some(rule) = &event.rule else {
            return if first < to { vec![first] } else { Vec::new() };
        };
        // Recurrences follow the wall clock of the zone the event was written
        // in, so a 10:00 meeting stays at 10:00 across a DST change.
        let (start, zone) = match event.start {
            When::Utc(at) => (at.to_zoned(TimeZone::UTC).datetime(), TimeZone::UTC),
            When::Local(datetime, zone) => (datetime, self.zone(zone, tz).clone()),
        };
        let until = rule.until.and_then(|until| self.instant(until, tz));
        let mut starts = Vec::new();
        let mut produced = 0usize;
        let first_step = if rule.count.is_some() {
            0
        } else {
            let from_date = from.to_zoned(zone.clone()).date();
            rule.steps_before(start.date(), from_date)
        };
        for date in rule.dates(start.date(), first_step).take(MAX_STEPS) {
            let Ok(at) = zone.to_zoned(date.to_datetime(start.time())) else {
                continue;
            };
            let at = at.timestamp();
            if at < first {
                continue;
            }
            if at >= to || until.is_some_and(|until| at > until) {
                break;
            }
            produced += 1;
            if rule.count.is_some_and(|count| produced > count) {
                break;
            }
            starts.push(at);
        }
        starts
    }

    fn zone<'a>(&'a self, zone: Option<Zone>, local: &'a TimeZone) -> &'a TimeZone {
        zone.and_then(|zone| self.zones.get(zone)).unwrap_or(local)
    }

    fn instant(&self, when: When, tz: &TimeZone) -> Option<Timestamp> {
        match when {
            When::Utc(at) => Some(at),
            When::Local(datetime, zone) => self
                .zone(zone, tz)
                .to_zoned(datetime)
                .ok()
                .map(|zoned| zoned.timestamp()),
        }
    }
}

impl Rule {
    /// Whole periods of the rule that end before `from`, less one, so
    /// skipping them can't skip a meeting.
    fn steps_before(&self, start: Date, from: Date) -> i64 {
        let days = i64::from(start.until(from).map_or(0, |span| span.get_days()));
        let months = (i64::from(from.year()) - i64::from(start.year())) * 12
            + i64::from(from.month())
            - i64::from(start.month());
        let periods = match self.frequency {
            Frequency::Daily => days,
            Frequency::Weekly => days / 7,
            Frequency::Monthly => months,
            Frequency::Yearly => months / 12,
        };
        (periods / i64::from(self.interval) - 1).max(0)
    }

    /// Candidate dates from period `first_step` on, in order, without an
    /// end: the caller stops at the range, the count or `until`.
    fn dates(&self, start: Date, first_step: i64) -> Box<dyn Iterator<Item = Date> + '_> {
        let interval = i64::from(self.interval);
        match self.frequency {
            Frequency::Daily => Box::new(
                (first_step..)
                    .map_while(move |step: i64| start.checked_add((step * interval).days()).ok())
                    .filter(|date| {
                        self.by_day.is_empty()
                            || self.by_day.iter().any(|&(_, day)| day == date.weekday())
                    }),
            ),
            Frequency::Weekly => {
                let monday = start
                    .checked_sub(i64::from(start.weekday().to_monday_zero_offset()).days())
                    .unwrap_or(start);
                let mut days: Vec<i8> = if self.by_day.is_empty() {
                    vec![start.weekday().to_monday_zero_offset()]
                } else {
                    self.by_day
                        .iter()
                        .map(|&(_, day)| day.to_monday_zero_offset())
                        .collect()
                };
                days.sort_unstable();
                days.dedup();
                Box::new(
                    (first_step..)
                        .map_while(move |step: i64| {
                            monday.checked_add((step * interval).weeks()).ok()
                        })
                        .flat_map(move |week| {
                            days.clone()
                                .into_iter()
                                .filter_map(move |day| week.checked_add(i64::from(day).days()).ok())
                        })
                        .filter(move |&date| date >= start),
                )
            }
            Frequency::Monthly => {
                let first = start.first_of_month();
                Box::new(
                    (first_step..)
                        .map_while(move |step: i64| {
                            first.checked_add((step * interval).months()).ok()
                        })
                        .flat_map(move |month| self.days_in(month, start))
                        .filter(move |&date| date >= start),
                )
            }
            Frequency::Yearly => Box::new(
                (first_step..)
                    .map_while(move |step: i64| {
                        i16::try_from(i64::from(start.year()) + step * interval).ok()
                    })
                    // February 29 only happens in leap years.
                    .filter_map(move |year| Date::new(year, start.month(), start.day()).ok()),
            ),
        }
    }

    /// The days of `month` a monthly rule picks, in order.
    fn days_in(&self, month: Date, start: Date) -> Vec<Date> {
        let mut days: Vec<Date> = if !self.by_day.is_empty() {
            self.by_day
                .iter()
                .flat_map(|&(nth, weekday)| match nth {
                    Some(nth) => month
                        .nth_weekday_of_month(nth, weekday)
                        .ok()
                        .into_iter()
                        .collect(),
                    None => (1..=month.days_in_month())
                        .filter_map(|day| month.with().day(day).build().ok())
                        .filter(|date| date.weekday() == weekday)
                        .collect::<Vec<_>>(),
                })
                .collect()
        } else {
            let wanted = if self.by_month_day.is_empty() {
                vec![start.day()]
            } else {
                self.by_month_day.clone()
            };
            let length = month.days_in_month();
            wanted
                .into_iter()
                .filter_map(|day| {
                    // -1 is the last day of the month.
                    let day = if day < 0 { length + 1 + day } else { day };
                    (1..=length).contains(&day).then_some(day)
                })
                .filter_map(|day| month.with().day(day).build().ok())
                .collect()
        };
        days.sort_unstable();
        days.dedup();
        days
    }
}

/// An event as read so far.
#[derive(Default)]
struct Draft {
    uid: Option<String>,
    start: Option<(String, Option<String>, bool)>,
    end: Option<(String, Option<String>, bool)>,
    duration: Option<String>,
    rule: Option<String>,
    except: Vec<(String, Option<String>)>,
    replaces: Option<(String, Option<String>)>,
    skip: bool,
}

impl Draft {
    fn add(&mut self, property: Property) {
        let tzid = property.param("TZID");
        let is_date = property
            .param("VALUE")
            .is_some_and(|value| value.eq_ignore_ascii_case("DATE"));
        let value = property.value;
        match property.name.as_str() {
            "UID" => self.uid = Some(value),
            "DTSTART" => self.start = Some((value, tzid, is_date)),
            "DTEND" => self.end = Some((value, tzid, is_date)),
            "DURATION" => self.duration = Some(value),
            "RRULE" => self.rule = Some(value),
            "EXDATE" => self.except.extend(
                value
                    .split(',')
                    .map(|part| (part.trim().to_string(), tzid.clone())),
            ),
            "RECURRENCE-ID" => self.replaces = Some((value, tzid)),
            // A cancelled meeting, or time marked free, keeps nobody busy.
            "STATUS" if value.eq_ignore_ascii_case("CANCELLED") => self.skip = true,
            "TRANSP" if value.eq_ignore_ascii_case("TRANSPARENT") => self.skip = true,
            _ => {}
        }
    }

    fn finish(self, zones: &mut Vec<TimeZone>, names: &mut Vec<String>) -> Option<Event> {
        let (start_text, start_zone, start_is_date) = self.start?;
        // All-day events are days off or reminders, not meetings.
        if start_is_date {
            return None;
        }
        let mut zone_of = |tzid: Option<String>| zone(tzid, zones, names);
        let start = parse_when(&start_text, zone_of(start_zone))?;
        let replaces = self
            .replaces
            .and_then(|(text, tzid)| parse_when(&text, zone_of(tzid)));
        if self.skip && replaces.is_none() {
            return None;
        }
        let length = match (self.end, self.duration) {
            (Some((text, tzid, false)), _) => {
                let end = parse_when(&text, zone_of(tzid))?;
                length_between(start, end)?
            }
            (_, Some(duration)) => parse_duration(&duration)?,
            _ => return None,
        };
        if length <= SignedDuration::ZERO {
            return None;
        }
        let event = Event {
            uid: self.uid,
            start,
            length,
            // A rule it can't follow keeps the first meeting, which is sure.
            rule: self.rule.and_then(|text| parse_rule(&text, &mut zone_of)),
            except: self
                .except
                .into_iter()
                .filter_map(|(text, tzid)| parse_when(&text, zone_of(tzid)))
                .collect(),
            replaces,
        };
        // A cancelled occurrence still hides the one it replaces, but keeps
        // nobody busy: a zero length drops it from the busy times.
        Some(if self.skip {
            Event {
                length: SignedDuration::ZERO,
                ..event
            }
        } else {
            event
        })
    }
}

/// The time zone a `TZID` names, if the app knows it. Unknown names, such
/// as Windows zone names some exports use, read as the computer's own zone.
fn zone(tzid: Option<String>, zones: &mut Vec<TimeZone>, names: &mut Vec<String>) -> Option<Zone> {
    let name = tzid?;
    let name = name.trim_start_matches('/');
    if let Some(index) = names.iter().position(|known| known == name) {
        return Some(index);
    }
    let tz = TimeZone::get(name).ok()?;
    names.push(name.to_string());
    zones.push(tz);
    Some(zones.len() - 1)
}

fn length_between(start: When, end: When) -> Option<SignedDuration> {
    match (start, end) {
        (When::Utc(start), When::Utc(end)) => Some(end.duration_since(start)),
        (When::Local(start, _), When::Local(end, _)) => Some(start.duration_until(end)),
        // Mixed forms: compare as UTC, close enough for a length.
        (When::Utc(start), When::Local(end, _)) => {
            Some(start.duration_until(end.to_zoned(TimeZone::UTC).ok()?.timestamp()))
        }
        (When::Local(start, _), When::Utc(end)) => Some(
            start
                .to_zoned(TimeZone::UTC)
                .ok()?
                .timestamp()
                .duration_until(end),
        ),
    }
}

/// `20261106T140000Z`, `20261106T140000`, or a date alone (midnight).
fn parse_when(text: &str, zone: Option<Zone>) -> Option<When> {
    let text = text.trim();
    let (text, utc) = match text.strip_suffix(['Z', 'z']) {
        Some(text) => (text, true),
        None => (text, false),
    };
    let (date, time) = match text.split_once(['T', 't']) {
        Some((date, time)) => (date, Some(time)),
        None => (text, None),
    };
    let date = parse_date(date)?;
    let time = match time {
        Some(time) if time.len() == 6 && time.bytes().all(|byte| byte.is_ascii_digit()) => {
            Time::new(
                time[0..2].parse().ok()?,
                time[2..4].parse().ok()?,
                // A leap second reads as the second before.
                time[4..6].parse::<i8>().ok()?.min(59),
                0,
            )
            .ok()?
        }
        Some(_) => return None,
        None => Time::midnight(),
    };
    let datetime = date.to_datetime(time);
    Some(if utc {
        When::Utc(datetime.to_zoned(TimeZone::UTC).ok()?.timestamp())
    } else {
        When::Local(datetime, zone)
    })
}

fn parse_date(text: &str) -> Option<Date> {
    if text.len() != 8 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Date::new(
        text[0..4].parse().ok()?,
        text[4..6].parse().ok()?,
        text[6..8].parse().ok()?,
    )
    .ok()
}

/// `PT1H30M`, `P1D`, `P1W`. A negative duration is refused.
fn parse_duration(text: &str) -> Option<SignedDuration> {
    let text = text.trim().strip_prefix('+').unwrap_or(text.trim());
    let mut rest = text.strip_prefix(['P', 'p'])?;
    let mut seconds: i64 = 0;
    let mut in_time = false;
    let mut any = false;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix(['T', 't']) {
            in_time = true;
            rest = after;
            continue;
        }
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let number: i64 = rest[..digits].parse().ok()?;
        let unit = rest[digits..].chars().next()?.to_ascii_uppercase();
        let scale = match (unit, in_time) {
            ('W', false) => 7 * 86_400,
            ('D', false) => 86_400,
            ('H', true) => 3_600,
            ('M', true) => 60,
            ('S', true) => 1,
            _ => return None,
        };
        seconds = seconds.checked_add(number.checked_mul(scale)?)?;
        any = true;
        rest = &rest[digits + 1..];
    }
    any.then(|| SignedDuration::from_secs(seconds))
}

/// `FREQ=WEEKLY;BYDAY=MO,WE;UNTIL=20261231T000000Z`. A rule with a part
/// this reader can't follow gives `None`, and the event keeps only its first
/// meeting.
fn parse_rule(
    text: &str,
    zone_of: &mut impl FnMut(Option<String>) -> Option<Zone>,
) -> Option<Rule> {
    let mut frequency = None;
    let mut rule = Rule {
        frequency: Frequency::Daily,
        interval: 1,
        count: None,
        until: None,
        by_day: Vec::new(),
        by_month_day: Vec::new(),
    };
    for part in text.split(';').filter(|part| !part.is_empty()) {
        let (key, value) = part.split_once('=')?;
        match key.to_ascii_uppercase().as_str() {
            "FREQ" => {
                frequency = Some(match value.to_ascii_uppercase().as_str() {
                    "DAILY" => Frequency::Daily,
                    "WEEKLY" => Frequency::Weekly,
                    "MONTHLY" => Frequency::Monthly,
                    "YEARLY" => Frequency::Yearly,
                    _ => return None,
                });
            }
            "INTERVAL" => rule.interval = value.parse().ok().filter(|&n: &i32| n >= 1)?,
            "COUNT" => rule.count = Some(value.parse().ok()?),
            "UNTIL" => rule.until = Some(parse_when(value, zone_of(None))?),
            "BYDAY" => {
                rule.by_day = value
                    .split(',')
                    .map(parse_by_day)
                    .collect::<Option<Vec<_>>>()?;
            }
            "BYMONTHDAY" => {
                rule.by_month_day = value
                    .split(',')
                    .map(|day| {
                        day.parse()
                            .ok()
                            .filter(|&day: &i8| day != 0 && day.abs() <= 31)
                    })
                    .collect::<Option<Vec<_>>>()?;
            }
            // The week start only matters for rules every few weeks on
            // several days; Monday, the default, is what calendars write.
            "WKST" => {}
            _ => return None,
        }
    }
    rule.frequency = frequency?;
    let ordinals = rule.by_day.iter().any(|(nth, _)| nth.is_some());
    let supported = match rule.frequency {
        Frequency::Daily | Frequency::Weekly => !ordinals && rule.by_month_day.is_empty(),
        Frequency::Monthly => rule.by_day.is_empty() || rule.by_month_day.is_empty(),
        Frequency::Yearly => rule.by_day.is_empty() && rule.by_month_day.is_empty(),
    };
    supported.then_some(rule)
}

/// `MO`, `2TU`, `-1FR`.
fn parse_by_day(text: &str) -> Option<(Option<i8>, Weekday)> {
    let text = text.trim();
    if text.len() < 2 {
        return None;
    }
    let (nth, day) = text.split_at(text.len() - 2);
    let day = match day.to_ascii_uppercase().as_str() {
        "MO" => Weekday::Monday,
        "TU" => Weekday::Tuesday,
        "WE" => Weekday::Wednesday,
        "TH" => Weekday::Thursday,
        "FR" => Weekday::Friday,
        "SA" => Weekday::Saturday,
        "SU" => Weekday::Sunday,
        _ => return None,
    };
    let nth = match nth.strip_prefix('+').unwrap_or(nth) {
        "" => None,
        nth => Some(nth.parse().ok().filter(|&n: &i8| n != 0 && n.abs() <= 5)?),
    };
    Some((nth, day))
}

/// Sorted, with overlapping and touching times joined.
pub fn merge(mut busy: Vec<(Timestamp, Timestamp)>) -> Vec<(Timestamp, Timestamp)> {
    busy.retain(|(start, end)| start < end);
    busy.sort_unstable();
    let mut merged: Vec<(Timestamp, Timestamp)> = Vec::with_capacity(busy.len());
    for (start, end) in busy {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// Content lines: a line that starts with a space or a tab continues the one
/// before (RFC 5545 §3.1).
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        match (raw.strip_prefix([' ', '\t']), lines.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => lines.push(raw.to_string()),
        }
    }
    lines
}

struct Property {
    /// Upper case.
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Property {
    /// `NAME;PARAM=value;PARAM="quoted:value":VALUE`.
    fn parse(line: &str) -> Option<Self> {
        let mut in_quotes = false;
        let colon = line.char_indices().find_map(|(index, char)| {
            match char {
                '"' => in_quotes = !in_quotes,
                ':' if !in_quotes => return Some(index),
                _ => {}
            }
            None
        })?;
        let (head, value) = (&line[..colon], &line[colon + 1..]);
        let mut parts = head.split(';');
        let name = parts.next()?.trim().to_ascii_uppercase();
        let params = parts
            .filter_map(|param| param.split_once('='))
            .map(|(key, value)| {
                (
                    key.trim().to_ascii_uppercase(),
                    value.trim_matches('"').to_string(),
                )
            })
            .collect();
        Some(Self {
            name,
            params,
            value: value.to_string(),
        })
    }

    fn param(&self, key: &str) -> Option<String> {
        self.params
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> Timestamp {
        paris().to_zoned(text.parse().unwrap()).unwrap().timestamp()
    }

    fn calendar(events: &str) -> Calendar {
        Calendar::parse(&format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{events}END:VCALENDAR\r\n"
        ))
    }

    fn busy(calendar: &Calendar, from: &str, to: &str) -> Vec<(String, String)> {
        let tz = paris();
        calendar
            .busy(at(from), at(to), &tz)
            .into_iter()
            .map(|(start, end)| {
                let show = |at: Timestamp| at.to_zoned(tz.clone()).datetime().to_string();
                (show(start), show(end))
            })
            .collect()
    }

    fn pair(start: &str, end: &str) -> (String, String) {
        (format!("{start}:00"), format!("{end}:00"))
    }

    #[test]
    fn reads_single_events_in_each_time_form() {
        let calendar = calendar(
            "BEGIN:VEVENT\r\nUID:a\r\nDTSTART:20261106T090000Z\r\nDTEND:20261106T093000Z\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:b\r\nDTSTART;TZID=America/New_York:20261106T080000\r\nDURATION:PT1H\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:c\r\nDTSTART:20261106T160000\r\nDTEND:20261106T163000\r\nEND:VEVENT\r\n",
        );
        assert_eq!(calendar.len(), 3);
        assert_eq!(
            busy(&calendar, "2026-11-06T00:00", "2026-11-07T00:00"),
            [
                pair("2026-11-06T10:00", "2026-11-06T10:30"),
                pair("2026-11-06T14:00", "2026-11-06T15:00"),
                pair("2026-11-06T16:00", "2026-11-06T16:30"),
            ]
        );
    }

    #[test]
    fn skips_all_day_free_cancelled_and_unreadable_events() {
        let calendar = calendar(
            "BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261106\r\nDTEND;VALUE=DATE:20261107\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nDTSTART:20261106T090000\r\nDTEND:20261106T100000\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nDTSTART:20261106T090000\r\nDTEND:20261106T100000\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nDTSTART:tomorrow\r\nDTEND:20261106T100000\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nDTSTART:20261106T090000\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nDTSTART:20261106T090000\r\nDTEND:20261106T080000\r\nEND:VEVENT\r\n",
        );
        assert!(calendar.is_empty());
    }

    #[test]
    fn folded_lines_quoted_params_and_alarms() {
        let calendar = calendar(
            "BEGIN:VEVENT\r\nSUMMARY:Stand\r\n up\r\nDTSTART;TZID=\"Europe/Paris\":2026\r\n 1106T091500\r\n\
             DTEND;TZID=Europe/Paris:20261106T093000\r\n\
             BEGIN:VALARM\r\nTRIGGER:-PT10M\r\nDTSTART:20200101T000000\r\nEND:VALARM\r\nEND:VEVENT\r\n",
        );
        assert_eq!(
            busy(&calendar, "2026-11-06T00:00", "2026-11-07T00:00"),
            [pair("2026-11-06T09:15", "2026-11-06T09:30")]
        );
    }

    #[test]
    fn weekly_meetings_keep_their_wall_clock_across_dst() {
        // Every Monday and Thursday at 10:00 Paris time, across the change
        // to winter time on 2026-10-25.
        let calendar = calendar(
            "BEGIN:VEVENT\r\nUID:w\r\nDTSTART;TZID=Europe/Paris:20261019T100000\r\nDTEND;TZID=Europe/Paris:20261019T103000\r\n\
             RRULE:FREQ=WEEKLY;BYDAY=MO,TH;COUNT=4\r\nEND:VEVENT\r\n",
        );
        assert_eq!(
            busy(&calendar, "2026-10-01T00:00", "2026-12-01T00:00"),
            [
                pair("2026-10-19T10:00", "2026-10-19T10:30"),
                pair("2026-10-22T10:00", "2026-10-22T10:30"),
                pair("2026-10-26T10:00", "2026-10-26T10:30"),
                pair("2026-10-29T10:00", "2026-10-29T10:30"),
            ]
        );
    }

    #[test]
    fn exceptions_and_moved_occurrences() {
        let calendar = calendar(
            "BEGIN:VEVENT\r\nUID:d\r\nDTSTART;TZID=Europe/Paris:20261102T090000\r\nDURATION:PT15M\r\n\
             RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR;UNTIL=20261106T235959Z\r\n\
             EXDATE;TZID=Europe/Paris:20261103T090000\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:d\r\nRECURRENCE-ID;TZID=Europe/Paris:20261104T090000\r\n\
             DTSTART;TZID=Europe/Paris:20261104T110000\r\nDTEND;TZID=Europe/Paris:20261104T113000\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:d\r\nRECURRENCE-ID;TZID=Europe/Paris:20261105T090000\r\n\
             DTSTART;TZID=Europe/Paris:20261105T090000\r\nDURATION:PT15M\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\n",
        );
        assert_eq!(
            busy(&calendar, "2026-11-01T00:00", "2026-11-15T00:00"),
            [
                pair("2026-11-02T09:00", "2026-11-02T09:15"),
                pair("2026-11-04T11:00", "2026-11-04T11:30"),
                pair("2026-11-06T09:00", "2026-11-06T09:15"),
            ]
        );
    }

    #[test]
    fn monthly_and_yearly_rules() {
        let calendar = calendar(
            "BEGIN:VEVENT\r\nUID:m\r\nDTSTART:20261030T150000\r\nDURATION:PT1H\r\n\
             RRULE:FREQ=MONTHLY;BYDAY=-1FR;COUNT=3\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:n\r\nDTSTART:20261031T080000\r\nDURATION:PT30M\r\n\
             RRULE:FREQ=MONTHLY;INTERVAL=1\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:y\r\nDTSTART:20240229T120000\r\nDURATION:PT1H\r\nRRULE:FREQ=YEARLY\r\nEND:VEVENT\r\n",
        );
        assert_eq!(
            busy(&calendar, "2026-11-01T00:00", "2027-01-31T00:00"),
            [
                pair("2026-11-27T15:00", "2026-11-27T16:00"),
                pair("2026-12-25T15:00", "2026-12-25T16:00"),
                // November has no 31st: that month is skipped.
                pair("2026-12-31T08:00", "2026-12-31T08:30"),
            ]
        );
        assert_eq!(
            busy(&calendar, "2028-02-01T00:00", "2028-03-01T00:00"),
            [pair("2028-02-29T12:00", "2028-02-29T13:00")]
        );
    }

    #[test]
    fn a_rule_it_cannot_follow_keeps_only_the_first_meeting() {
        let calendar = calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261102T090000\r\nDURATION:PT1H\r\n\
             RRULE:FREQ=MONTHLY;BYDAY=MO;BYSETPOS=1\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nDTSTART:20261102T120000\r\nDURATION:PT1H\r\n\
             RRULE:FREQ=HOURLY\r\nEND:VEVENT\r\n",
        );
        assert_eq!(
            busy(&calendar, "2026-11-01T00:00", "2026-12-31T00:00"),
            [
                pair("2026-11-02T09:00", "2026-11-02T10:00"),
                pair("2026-11-02T12:00", "2026-11-02T13:00"),
            ]
        );
    }

    #[test]
    fn an_unknown_zone_name_reads_as_local_time() {
        let calendar = calendar(
            "BEGIN:VEVENT\r\nDTSTART;TZID=Romance Standard Time:20261106T090000\r\nDURATION:PT1H\r\nEND:VEVENT\r\n",
        );
        assert_eq!(
            busy(&calendar, "2026-11-06T00:00", "2026-11-07T00:00"),
            [pair("2026-11-06T09:00", "2026-11-06T10:00")]
        );
    }

    #[test]
    fn overlapping_meetings_merge_and_old_rules_stay_cheap() {
        let calendar = calendar(
            "BEGIN:VEVENT\r\nDTSTART:19900101T100000\r\nDURATION:PT1H\r\nRRULE:FREQ=DAILY\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nDTSTART:20261106T103000\r\nDURATION:PT1H\r\nEND:VEVENT\r\n",
        );
        // The daily rule from 1990 skips ahead to the range instead of
        // stepping through 36 years, and joins the meeting it overlaps.
        assert_eq!(
            busy(&calendar, "2026-11-06T00:00", "2026-11-07T00:00"),
            [pair("2026-11-06T10:00", "2026-11-06T11:30")]
        );
        assert_eq!(
            merge(vec![
                (at("2026-11-06T09:00"), at("2026-11-06T10:00")),
                (at("2026-11-06T10:00"), at("2026-11-06T10:30"))
            ]),
            [(at("2026-11-06T09:00"), at("2026-11-06T10:30"))]
        );
    }

    #[test]
    fn durations() {
        assert_eq!(
            parse_duration("PT1H30M"),
            Some(SignedDuration::from_mins(90))
        );
        assert_eq!(
            parse_duration("P1DT2H"),
            Some(SignedDuration::from_hours(26))
        );
        assert_eq!(parse_duration("P1W"), Some(SignedDuration::from_hours(168)));
        assert_eq!(parse_duration("-PT1H"), None);
        assert_eq!(parse_duration("PT"), None);
        assert_eq!(parse_duration("P1H"), None);
    }
}

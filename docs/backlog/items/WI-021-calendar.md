---
id: WI-021
title: Turn Unidle on during meetings from a local calendar file
type: feature
status: review
branch: feature/WI-021-calendar
created: 2026-11-06
updated: 2026-11-06
related: [WI-009, WI-020]
epic:
children: []
---

## Proposal (rationale)

Phase 4's Could row for calendar awareness: ON during meetings, for example so the screen stays on through a call. The roadmap left open whether a calendar URL was worth network access.

## Decision

`accepted` — 2026-11-06, Antoine ("Local .ics file only" on the calendar network question). No network: the app reads a file, and an online calendar reaches it through the calendar app's own export or sync.

## Plan

### Requirements

- SPEC §18, CA-1 to CA-5.
- Meetings plan ON times the way schedule windows do, with the same manual and timer precedence (§10).

### Impacts

- `crates/core`: `calendar.rs` reads iCalendar text into busy times; `Automation` merges them with the schedule; `Source::Calendar`, `NextChange::source`, `Status::paused_by`; a `calendarFile` setting.
- `src-tauri`: `calendar.rs` follows the file on a thread and hands the controller the busy times; the file dialog; source and pause wording.
- `ui`: a Calendar group in Settings, and the Meeting source line.

### Approach

- **A small reader, not a dependency.** Only start, end or duration, the recurrence rule and its exceptions, and free or cancelled matter. A rule beyond the supported parts keeps its first meeting rather than guessing.
- **Busy times, not events, in the controller.** The app works out two weeks of merged times every hour or when the file changes, so a tick only searches a sorted list.
- **The plan is schedule or calendar.** Everything that compared against the schedule (manual pauses, re-anchoring, the next change) now compares against both.

### Acceptance criteria / tests

1. Unit tests: event forms and zones, skipped events, folding and alarms, weekly rules across DST, exceptions and moved occurrences, monthly and yearly rules, unsupported rules, old rules staying cheap; meetings in the automation (source, next change, pause, re-anchoring) and the controller; settings; the source line.
2. Linux end to end: a settings file pointing at an `.ics` with a meeting now starts ON with source `calendar`; rewriting the file to move the meeting turns it OFF at the next check and shows "Meeting · ON at …".
3. Screenshot of the Calendar group.
4. On real machines: files exported from Outlook, Google Calendar and Apple Calendar. **Needs real machines.**

### Out of scope

- Fetching a calendar URL, and reading calendar apps' own stores.
- Windows zone names in `TZID` (they read as the computer's zone).

## Execution

- Branch `feature/WI-021-calendar`, stacked on `feature/WI-020-event-scripts`.

## Review

1. Met. `cargo test --workspace`.
2. Met.
3. Met.
4. **Open.** Needs real machines.

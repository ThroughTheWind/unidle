---
id: WI-016
title: Keep an opt-in local activity log of ON sessions, with CSV export
type: feature
status: review
branch: feature/WI-016-activity-log
created: 2026-11-05
updated: 2026-11-05
related: [WI-014, WI-015]
epic:
children: []
---

## Proposal (rationale)

Phase 4's activity log row: a local history of sessions with CSV export. People who keep their machine awake for work want to show when, and why, without trusting a vendor's dashboard; the risk notes for work machines ask for the app to be transparent about what it did.

## Decision

`accepted` — 2026-11-05, Antoine ("continue with the rest of phase 4").

## Plan

### Requirements

- SPEC §15, AL-1 to AL-4.
- Off by default; nothing is written until the user turns it on.
- One row per ON session, ending on OFF, a condition hold, sleep (at the time it fell asleep) or quitting.

### Impacts

- `crates/core`: `activity.rs` (the session row, the CSV file with a row cap), controller session tracking and a session sink, `activityLog` setting.
- `src-tauri`: `activity_log.rs` installs the sink when the setting is on, and the export and clear commands.
- `ui`: an Activity log group in Settings.

### Approach

- **A sink, not a drain.** The controller hands each finished session to a callback. Sessions end in many places (a click, the CLI, a tick, quitting), and a callback set once covers all of them, including the controller being dropped.
- **Sessions, not events.** The in-memory event log (SPEC §11) holds nudges and skips; writing those would grow fast and say more than anyone needs. A session row carries the nudge count instead.
- **Wall-clock times from the same `Zoned` the controller decides with**, so tests drive them and a row's times agree with the schedule.

### Acceptance criteria / tests

1. Unit tests: the CSV row in local time with its offset; one header and the row cap; the file created, read and cleared; a session from ON to OFF counting nudges; a hold ending one session and resuming another; sleep ending it when the computer fell asleep; quitting ending it; `unidle on` making the session power-only; the setting's default and round trip.
2. The Settings group, by eye, light and dark.
3. On a real build: turn on the log, run a few sessions, export, open in a spreadsheet. **Needs a real machine.**

### Out of scope

- Showing the log inside the app. The CSV opens in any spreadsheet.
- Logging condition holds as their own rows: a hold ends a session, and the gap says the rest.

## Execution

- Branch `feature/WI-016-activity-log`, stacked on `feature/WI-015-conditions` (the controller's held state).

## Review

1. Met. `cargo test --workspace`; core clippy clean for `x86_64-pc-windows-gnu` and `x86_64-apple-darwin`.
2. Met. Checked by eye.
3. **Open.** Needs a real machine.

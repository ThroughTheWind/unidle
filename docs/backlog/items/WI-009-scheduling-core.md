---
id: WI-009
title: Decide ON/OFF between manual, timers and a weekly schedule in the core
type: feature
status: review
branch: feature/WI-009-scheduling-core
created: 2026-11-04
updated: 2026-11-04
related: [WI-010, WI-011, WI-012]
epic:
children: []
---

## Proposal (rationale)

Phase 2 of the roadmap: the app turns itself on and off. Timers, schedules and the tray all need one place that decides who wins, and that place has to be right across DST changes and sleep/resume, or the machine stays awake all night or sleeps mid-presentation. SPEC §11 puts that in the controller.

## Decision

`accepted` — 2026-11-04, Antoine ("build phase 2"), as the first part of Phase 2.

## Plan

### Requirements

- Weekly windows per weekday, several per day, overnight windows, skip days (SPEC SC-1, SC-2).
- Timers until a wall-clock time (SC-3).
- The precedence in SPEC SC-5.
- The state, the source and the next change are computed from the wall clock and the system time zone, so DST and sleep/resume can't leave a stale state (SC-6).
- Quitting and dropping the controller release, whatever the schedule wants (SC-7).
- The schedule lives in `settings.json`: `scheduleEnabled`, `scheduleWindows`, `skipDates`, read leniently like the other fields (FR-15).
- The window and tray catch up when a timer or the schedule changes the state on its own.

### Impacts

- `crates/core`: new `schedule.rs` (windows, skip days, boundaries) and `automation.rs` (precedence), `Source::Timer` and `Source::Schedule`, `Status::next_change` and `paused_until`, a change callback on `Ticker`. `turn_on` no longer takes a source: it is the manual action.
- New dependency `jiff` for time zones and DST. Rust's standard library has no time zone support, and `jiff` reads the OS time zone database (bundled on Windows).
- `src-tauri`: the schedule is applied with the other settings, and automatic changes refresh the tray, the window and the optional notification.
- SPEC §10 is rewritten from "proposed" to the rules as built.

### Approach

- Pure functions of a wall-clock time: nothing is stepped through or caught up after a sleep. A monotonic timer can stop while the machine sleeps, so the ticker caps its wait at 15 s while anything is scheduled, then asks the wall clock.
- A window's local times are resolved in the time zone each day. A time skipped by a DST jump moves forward with it, and a repeated time takes its first occurrence.
- Manual and timer are one override slot, so the latest user action is the one that counts. A manual action that matches the schedule clears the override instead of pinning it.
- The next change is found by evaluating the decision at each candidate boundary (window edges, override end) in order, so overlapping windows and handovers need no special cases.

### Acceptance criteria / tests

1. Unit tests for windows: in and out, weekend, overnight, skip days, overlap, DST in both directions, a start inside the DST gap.
2. Unit tests for precedence: the SC-5 cases one by one, including a timer handing over to a window without a gap and a manual ON re-anchored by a new schedule.
3. Controller tests: scheduled ON and OFF on tick, a pause, a timer that ends during a sleep, a refused timer, a refused scheduled ON logged once and retried, drop releases.
4. Settings tests: the schedule round-trips, reads hand-written JSON, and a bad window or date falls back alone.
5. On each OS with a real build, a window starting a minute from now turns the app ON within 15 s of the start, and the tray follows. **Needs real machines.**

### Out of scope

- The tray and window controls for timers and the source line: WI-010.
- The schedule editor in Settings: WI-011.
- Import and export: WI-012.
- Listening for OS resume events. The 15 s check covers it at no measurable cost; revisit if a real machine shows a longer delay.

## Execution

- Branch `feature/WI-009-scheduling-core`, off `main`.

## Review

1. Met. `cargo test -p unidle-core`: 11 schedule tests.
2. Met. 11 automation tests.
3. Met. 9 new controller tests; the 10 existing ones pass unchanged apart from `turn_on()` losing its argument.
4. Met. 2 new settings tests, and the round trip and fallback tests cover the new fields.
5. **Open.** Needs real machines.

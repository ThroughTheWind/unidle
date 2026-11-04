---
id: WI-010
title: Add quick timers and the source line to the tray and window
type: feature
status: review
branch: feature/WI-010-timers-and-source
created: 2026-11-04
updated: 2026-11-04
related: [WI-009, WI-011]
epic:
children: []
---

## Proposal (rationale)

WI-009 decides between manual, timers and the schedule, but nothing lets the user start a timer or see why the app is ON. ROADMAP Phase 2 lists quick timers in the tray, the state source indicator and the "next change" preview.

## Decision

`accepted` — 2026-11-04, Antoine ("build phase 2").

## Plan

### Requirements

- Tray: a "Keep awake for" submenu with 15 minutes, 30 minutes, 1 hour, 2 hours and "Until a time…" (SPEC SC-3). "Until a time…" opens the window on a time picker, since a tray menu can't take text.
- Window: the same timers in a row under the button, and an "Until…" time picker in place.
- Tray and window show the source and next change: "Timer · OFF at 18:00", "Scheduled · ON tomorrow at 09:00", "Schedule paused until 18:00", "Manual" (SPEC SC-4). The tray line appears only when there is something to say.
- A timer that can't start says why, like a failed toggle.

### Impacts

- `crates/core`: `next_local_time` for "until 18:00".
- `src-tauri`: `start_timer` and `take_timer_picker_request` commands, `StatusDto` gains `source`, `nextChange` and `pausedUntil`, tray submenu and detail line, `strings::detail`.
- `ui`: the timer row, the until picker, the detail line, `when.ts` for times.

### Approach

- The window formats times itself (strings stay in `strings.ts`, NFR-5), with the same 24-hour rules as the tray.
- The detail line uses `status-scheduled` blue, which the design system reserves for scheduling. A plain manual ON stays muted.

### Acceptance criteria / tests

1. Unit test: the tray's detail line for each source, today, tomorrow, this week, later, and paused.
2. Screenshots of the window OFF, ON by timer, OFF with a scheduled ON (dark), and the until picker, with a mocked Tauri runtime.
3. On each OS with a real build: each tray timer turns ON and shows "Timer · OFF at …"; "Until a time…" opens the picker whether the window was open or closed; the timer turns OFF on time. **Needs real machines.**

### Out of scope

- The schedule editor: WI-011.
- Remembering timers across a restart (SPEC SC-7).
- A 12-hour clock: the app uses 24-hour times everywhere until localization (Phase 5).

## Execution

- Branch `feature/WI-010-timers-and-source`, stacked on WI-009.

## Review

1. Met. `cargo test -p unidle` passes `detail_names_the_source_and_the_next_change`.
2. Met. Checked by eye in light and dark; the time input follows the OS locale's clock in its own field.
3. **Open.** Needs real machines.

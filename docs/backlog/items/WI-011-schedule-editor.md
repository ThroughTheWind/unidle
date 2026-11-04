---
id: WI-011
title: Edit the weekly schedule and days off in Settings
type: feature
status: review
branch: feature/WI-011-schedule-editor
created: 2026-11-04
updated: 2026-11-04
related: [WI-009, WI-010, WI-012]
epic:
children: []
---

## Proposal (rationale)

WI-009 stores and runs a weekly schedule, but the only way to set one is editing `settings.json`. ROADMAP Phase 2 lists weekly schedules (Must) and skip days (Should).

## Decision

`accepted` — 2026-11-04, Antoine ("build phase 2").

## Plan

### Requirements

- A Schedule group in Settings: a "Turn on automatically" switch, a list of time windows (days, start, end, remove), "Add a time", and days off (list, remove, add by date) (SPEC SC-1, SC-2).
- Turning the schedule on with no windows starts from Monday to Friday, 09:00 to 18:00. A new window copies the last one.
- A window can't lose its last day; the screen says why.
- An overnight window says it ends the next day.
- Every change is validated by `update_settings` and the screen redraws from what was kept.

### Impacts

- `ui`: `schedule.ts`, the group in `index.html`, strings and styles. No Rust change: the fields and validation are WI-009's.

### Approach

- Day toggles are buttons with `aria-pressed`, filled green when in, so the state reads by fill and weight as well as color (NFR-4).
- Native `<input type="time">` and `<input type="date">`, which follow the OS locale and come with keyboard support. The settings file always stores 24-hour `HH:MM` and ISO dates.

### Acceptance criteria / tests

1. Screenshots with a mocked runtime: windows with a 12-hour locale (AM/PM fit), an overnight window, days off, dark theme, and the empty state.
2. `npm run build:ui` passes.
3. On each OS with a real build: add, change and remove windows and days off; the file holds them; a window starting a minute from now turns the app ON. **Needs real machines.**

### Out of scope

- Import and export: WI-012.
- Recurring days off (every December 25th): add when someone asks.

## Execution

- Branch `feature/WI-011-schedule-editor`, stacked on WI-010.
- The first layout had "From" and "to" labels; with a 12-hour locale they cut off AM/PM, so the times sit side by side with a dash and the labels moved to `aria-label`.

## Review

1. Met. Checked by eye.
2. Met.
3. **Open.** Needs real machines.

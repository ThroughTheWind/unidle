---
id: WI-013
title: Tidy the main screen and restyle the window's controls
type: feature
status: review
branch: claude/project-thread-uw5zsp
created: 2026-11-04
updated: 2026-11-04
related: [WI-007, WI-010, WI-011]
epic:
children: []
---

## Proposal (rationale)

After trying the Phase 2 build, Antoine wants the main screen calmer and the form controls closer to the design: the timer row should be optional, the "Unidle" label is redundant under the OS title bar, and the select and time picker look like OS defaults.

## Decision

`accepted` — 2026-11-04, Antoine, in the project thread.

## Plan

### Requirements

- A "Show timers on the main screen" setting, on by default, stored as `showTimers`. A missing or invalid value falls back to on.
- The main screen shows no "Unidle" label. Settings keeps its title.
- The select and time fields follow the design, including their popovers, and the other controls get clearer hover states.

### Impacts

- `crates/core/src/settings.rs`, the window in `ui/`, SPEC SC-3 and the design system.
- Existing settings files keep working: the field is new and defaults to on.

### Approach

- The select uses `appearance: base-select` where the webview supports it, with no extra script. Other webviews keep the OS list behind the same styled field.
- The time field keeps the native input for typing and adds a small fixed-position popover, since the OS picker can't be styled and WebKit on macOS and Linux has none.
- The tray's "Until a time…" still opens the picker while the row is hidden.

### Acceptance criteria / tests

- `cargo test -p unidle-core` covers the default, the fallback and the round trip of `showTimers`.
- With the setting off, the row is gone and the tray's "Until a time…" still opens the picker.
- Picking a time updates the field and sends one change; Escape restores the old time without leaving the form or Settings.
- Popovers stay inside the 360 × 420 window, in both themes.

### Out of scope

- A custom calendar for days off: it keeps the native date popup, which follows the theme. Nothing deferred.
- The landing page's demo window, which still shows the name.

## Execution

Branch `claude/project-thread-uw5zsp`, on top of the landing page commit 5eda765.

With the select drawing its own list, Escape reached the window's handler and left Settings, so that handler now skips an open select.

## Review

Checked in Edge at 360 × 420 against a stand-in for the Rust side, not in the running app:

- `cargo test -p unidle-core`: `showTimers` default, fallback and round trip pass.
- Setting off: the row is hidden. The tray's request wasn't exercised; `renderTimers` keeps the picker open while the row is hidden.
- Picking 10 then 30 gives 10:30 and one settings update, with focus back on the clock button. Escape restored 09:00 and stayed in Settings; in the Until form it kept the form open.
- Popovers stayed inside the window in the dark and light themes.
- Not checked: macOS and Linux webviews, including their OS select list, and 12-hour locales.


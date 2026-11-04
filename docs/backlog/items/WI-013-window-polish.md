---
id: WI-013
title: Let the timer row and tray click be turned off, drop the main title, polish form controls
type: feature
status: review
branch: feature/WI-013-window-polish
created: 2026-11-04
updated: 2026-11-04
related: [WI-010, WI-011]
epic:
children: []
---

## Proposal (rationale)

Antoine tried Phase 2 locally and asked for four things. A setting to show or hide the timer row on the main screen. No "Unidle" label on the main screen. A setting for whether clicking the tray icon turns the app ON or OFF. Better-looking selects, time and date pickers.

## Decision

`accepted` — 2026-11-04, Antoine.

## Plan

### Requirements

- `showTimers` (default on): the timer row under the button. The tray keeps its timers either way, and a picker the tray asks for still opens.
- `trayClickToggles` (default on for Windows and Linux, off on macOS, SPEC FR-2): a left click on the tray icon turns the app ON or OFF, and a right click opens the menu. When off, a left click opens the menu.
- The main screen's bar has no title. Settings still shows "Settings", and the OS title bar still names the app.
- Selects get their own chevron. Time and date fields get a quieter picker button and a filled highlight on the part being edited. Native popovers follow the window's theme (`color-scheme`) and use the brand green (`accent-color`). Inputs and secondary buttons get hover states.

### Impacts

- `crates/core` settings: two fields. `src-tauri`: `tray::apply_click`. `ui`: settings rows, strings and styles.

### Approach

- The native popovers (the select's list, the clock and the calendar) are kept, not rebuilt. They are the OS's, keyboard- and screen-reader-ready. Theming them through `color-scheme` and `accent-color` fixes the light window on a dark OS, which showed dark popovers.

### Acceptance criteria / tests

1. Settings tests cover the new fields' defaults and round trip.
2. Screenshots: no title on the main screen, the timer row hidden, the custom select focused in dark, and the time and date fields.
3. On each OS with a real build, the tray click setting does what it says. **Needs real machines.**

### Out of scope

- Custom-built popovers. Revisit if the native ones look wrong on a real machine.

## Execution

- Branch `feature/WI-013-window-polish`, off `main`.

## Review

1. Met.
2. Met. Checked by eye.
3. **Open.** Needs real machines.

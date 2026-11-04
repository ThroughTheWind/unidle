---
id: WI-014
title: Make turning on or off from a tray icon click a setting
type: feature
status: review
branch: claude/project-thread-uw5zsp
created: 2026-11-04
updated: 2026-11-04
related: [WI-004]
epic:
children: []
---

## Proposal (rationale)

Antoine asked for an ON/OFF toggle on tray icon click. Left-click already toggled on Windows and on Linux trays that report clicks, with no way to turn it off, and macOS always opened the menu. A setting gives every OS the choice.

## Decision

`accepted` — 2026-11-04, Antoine, in the project thread. The request was for a toggle; reading it as a setting that keeps today's defaults is this item's interpretation, and the report back says so.

## Plan

### Requirements

- A "Click the tray icon to turn on or off" setting, stored as `trayClickToggles`.
- Defaults keep today's behaviour: on for Windows and Linux, off for macOS. A missing or invalid value falls back to the default.
- With it off, a left-click opens the tray menu. The change applies without a restart.

### Impacts

- `crates/core/src/settings.rs`, `src-tauri/src/tray.rs`, the Settings screen, SPEC FR-2.
- Existing settings files keep working: the field is new and takes the OS default.

### Approach

- Drive `show_menu_on_left_click` and the click handler from the setting. Tauri has no such control on Linux, where most trays only ever open the menu, so the hint says so.

### Acceptance criteria / tests

- `cargo test -p unidle-core` covers the default, the fallback and the round trip.
- On Windows, clicking the icon toggles with the setting on and opens the menu with it off.

### Out of scope

- Other click gestures, such as double-click or middle-click. Nothing deferred.

## Execution

Branch `claude/project-thread-uw5zsp`, on top of WI-013.

## Review

Core tests, clippy and the UI build pass. The tray click itself still needs a manual check on Windows and macOS: it can't be driven from a test here, and the running app is a separate build.

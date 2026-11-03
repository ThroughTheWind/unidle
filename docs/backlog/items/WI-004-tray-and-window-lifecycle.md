---
id: WI-004
title: Add the tray icon and hide-to-tray window lifecycle
type: feature
status: review
branch: feature/WI-004-tray-and-window
created: 2026-11-03
updated: 2026-11-03
related: [WI-003, WI-002]
epic:
children: []
---

## Proposal (rationale)

The app is meant to live in the tray (SPEC FR-2, FR-4, FR-12, FR-13), and the skeleton only has a window. Closing the window must release the webview, because the spec's memory limit depends on it (SPEC NFR-1). WI-003 also needs this in order to measure hidden memory.

## Decision

`accepted` — 2026-11-03, Antoine, as the third step of Phase 1.

## Plan

### Requirements

- A tray / menu bar icon from launch, whose shape differs between ON and OFF, with the tooltip "Unidle: ON" or "Unidle: OFF" (FR-4).
- Left-click toggles on Windows and Linux; on macOS the click opens the menu (FR-2).
- Tray menu: a status line with the elapsed time while ON (FR-5), "Turn ON" / "Turn OFF", "Open window", "Quit" (FR-2, FR-13, design system §6).
- Closing the window destroys it and its webview; the app stays in the tray (FR-12, NFR-1). "Open window" builds a new one.
- The first time the window is closed, ever, an OS notification says "Still running in the tray" (FR-12).
- Quit from the tray, or Cmd+Q on macOS, turns OFF first, then exits (FR-13). Any exit releases the assertion.
- The window follows changes made from the tray.

### Impacts

- `src-tauri`: new `tray.rs`, `window.rs` and `strings.rs`. Tauri features `tray-icon` and `image-png`; new plugin `tauri-plugin-notification`.
- Tray images in `src-tauri/icons/tray/`: colored for Windows and Linux, template images for macOS.
- `ui`: listens for `status-changed`.
- `AGENTS.md`: the Rust side's text lives in `src-tauri/src/strings.rs`.

### Approach

- Let the close go through, so Tauri destroys the window, and keep the process alive by preventing the exit request that follows when the last window closes. Only an explicit quit sets a flag that lets the exit through.
- macOS gets its own app menu, so Cmd+Q goes through the same quit path as the tray. The Dock icon reopens the window.
- The "notice shown" flag is a marker file in the app config directory, because the settings file doesn't exist yet. The settings item can fold it in.
- Deviation from design system §6: the menu uses the spec's "Turn ON" / "Turn OFF" labels instead of a checkmark item, and adds a status line for FR-5.

### Acceptance criteria / tests

1. `strings::status_on` reads "ON for less than a minute", "12 min", "1 h", "1 h 12 min" (unit test).
2. On Linux (Xvfb with openbox), the app starts with its window. Closing the window keeps the process running and ends the `WebKitWebProcess`.
3. On each OS, the tray icon appears at launch and swaps between the outline and filled shapes when toggled. The tooltip and menu follow.
4. On Windows, left-click on the icon toggles. On macOS, a click opens the menu.
5. On each OS, "Open window" after a close shows a working window, and the first close ever shows the notification once.
6. On each OS, Quit while ON exits and releases the assertion (`powercfg /requests`, `pmset -g assertions`, `systemd-inhibit --list`). On macOS, Cmd+Q does the same.

### Out of scope

- A single-instance guard: F-004.
- Showing an error when turning ON fails: F-003.
- Measuring memory with the window closed on each OS: WI-003.
- Starting hidden in the tray at login: the settings item, with launch at login.

## Execution

- Branch `feature/WI-004-tray-and-window`, stacked on WI-002's branch.
- The tray images are drawn as SVG in the same pulse glyph as the app icon and rendered to 32 px PNGs. The SVG sources sit beside them.

## Review

1. Met. `cargo test -p unidle` passes.
2. Met. On Xvfb with openbox, from a debug build: after closing the window the process stays and `WebKitWebProcess` exits. Private memory went from about 159 MB (app 37, web 113, network 9) to about 46 MB (app 37, network 9). A release build should be lower; WI-003 owns the real numbers.
3. **Open.** Xvfb has no tray host. Needs a real desktop.
4. **Open.** Same.
5. **Partly met.** Reopening needs the tray, so it is untested. The notification path ran without error on Linux.
6. **Open.** Needs real machines.

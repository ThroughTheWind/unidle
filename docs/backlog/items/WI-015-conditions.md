---
id: WI-015
title: Hold activity on conditions: screen lock, sleep, AC power, battery, a running app
type: feature
status: review
branch: feature/WI-015-conditions
created: 2026-11-05
updated: 2026-11-05
related: [WI-009, WI-014]
epic:
children: []
---

## Proposal (rationale)

Phase 4's condition rows: stop when the screen locks or the user chooses Sleep (Must), only on AC power and stop below N % battery (Should), only while an app runs (Should). They share one mechanism: something wants it ON, a condition says not now.

## Decision

`accepted` — 2026-11-05, Antoine ("continue with the rest of phase 4").

## Plan

### Requirements

- SPEC §14, CO-1 to CO-5, and the five settings in its table.
- A held state in the controller, separate from OFF: the assertion is released, the source is remembered by the automation, and the next tick that finds the condition clear turns it back ON.
- The window and tray say "Paused" and the reason; the toggle while held turns OFF.

### Impacts

- `crates/core`: `conditions.rs` (settings and the pure decision), `platform/environment.rs` (the OS readings on all three OSes), adapter methods with `None` defaults, controller `Held` state, `Status::held`, `Status::is_wanted`, settings fields.
- `src-tauri`: status DTO, tray line, CLI. `ui`: a Conditions group in Settings, the paused state.
- New `windows-sys` features: ToolHelp, StationsAndDesktops, WindowsProgramming.

### Approach

- **Lock is two things.** Nudging a locked screen never helps anyone, so nudges always stop (CO-2). Releasing the assertion too would let a locked machine sleep through a render, so that is a setting, off by default.
- **Sleep is detected, not subscribed to.** Two clocks, one counting sleep, give the time asleep without a window, a message loop or a D-Bus signal subscription, and are cheap enough to read on every tick.
- **Polling, not events,** every 5 s while ON or held, and only for the conditions that are set. Events (WTS session notifications, `NSWorkspace` notifications, logind signals) need a window or a run loop per OS; polling needs neither and the cost is a few reads every 5 s.

### Acceptance criteria / tests

1. Unit tests: the decision for each condition, unknown readings, ordering; the controller holding and resuming on power and on an app, nudges skipped while locked, release while locked, OFF after sleep (and not after a blip, and not when turned off), sleep during a schedule window pausing it; settings round trip and lenient reading; sysfs power supplies including a peripheral battery and USB-C.
2. Linux end to end (Xvfb, private D-Bus, a fake ScreenSaver): `unidle on` with the app condition unmet is held; starting the app resumes within 5 s; locking with "let it sleep" holds and releases the inhibitor; unlocking resumes; the app quitting holds again.
3. Screenshots: the Conditions group in Settings, the paused main screen.
4. On Windows and macOS with real builds: lock, unplug, low battery, a named app, and Sleep. **Needs real machines.**

### Out of scope

- Wi-Fi and VPN conditions (a Could row): a later item.
- A dimmer look for the paused button: the word "Paused" and the line under it carry the state for now.

## Execution

- Branch `feature/WI-015-conditions`, stacked on `feature/WI-014-cli` (both touch the controller and the status DTO).

## Review

1. Met. `cargo test --workspace`; core clippy clean for `x86_64-pc-windows-gnu` and `x86_64-apple-darwin`.
2. Met. Run by hand; output in the PR.
3. Met. Checked by eye, light and dark.
4. **Open.** Needs real machines.

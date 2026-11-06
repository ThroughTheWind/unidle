---
id: WI-020
title: Run the user's commands when Unidle turns on or off
type: feature
status: review
branch: feature/WI-020-event-scripts
created: 2026-11-05
updated: 2026-11-05
related: [WI-016, WI-019]
epic:
children: []
---

## Proposal (rationale)

Phase 4's Could row for run-on-event scripts: user commands that run on ON and OFF (opt-in, local only), for example to set a chat status, pause a sync, or log to the user's own tool.

## Decision

`accepted` — 2026-11-05, Antoine ("Build the extras now" on the Phase 4 extras question).

## Plan

### Requirements

- SPEC §17, SR-1 to SR-4.
- The scripts run on the real assertion changes, not on clicks: a hold, sleep and quitting count as OFF.

### Impacts

- `crates/core`: `Change` and a change sink in the controller, next to the activity log's session sink; `onCommand` and `offCommand` settings.
- `src-tauri`: `scripts.rs` runs them. `ui`: a Scripts group in Settings.

### Approach

- **The same hook as the activity log.** Sessions open and close exactly when the assertion is taken and let go, so the change sink fires at those two points and covers every path, including dropping the controller at exit.
- **Fire and forget.** The script is spawned and reaped on its own thread; the controller's lock is held only for the spawn.
- **The user's shell,** so a line works as it does in their terminal, and no window flashes on Windows.

### Acceptance criteria / tests

1. Unit tests: changes follow the assertion through ON, a repeated ON, a hold, a resume and quitting; the shell gets the line and the variables; settings read a blank command as none.
2. Screenshot of the Scripts group.
3. On real machines: a script on each OS, including `cmd` quoting on Windows. **Needs real machines.**

### Out of scope

- Showing script output or failures in the window.

## Execution

- Branch `feature/WI-020-event-scripts`, stacked on `feature/WI-019-automation-hooks`.

## Review

1. Met. `cargo test --workspace`.
2. Met.
3. **Open.** Needs real machines.

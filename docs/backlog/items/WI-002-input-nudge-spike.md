---
id: WI-002
title: Prove pointer and keystroke nudges on all three platforms
type: spike
status: review
branch: feature/WI-002-input-nudge
created: 2026-11-03
updated: 2026-11-03
related: [WI-001, ADR-0001, ADR-0003]
epic:
children: []
---

## Proposal (rationale)

Nudges must have no visible effect (SPEC FR-7, FR-9) and need the macOS Accessibility permission and a Wayland fallback (SPEC PB-2). `enigo` is the candidate crate and its Wayland support is experimental. Check SPEC acceptance criteria 3, 4, 5 and 7, including whether real user input can be detected so nudges can be skipped (FR-8).

## Decision

`accepted` — 2026-11-03, Antoine, as the second step of Phase 1.

## Plan

### Requirements

- While ON with nudges enabled, one nudge per interval (default 60 s, range 10 s to 10 min, ±20% jitter), by pointer, keystroke or both alternating (FR-7, §4, §5.4).
- Pointer: 1 px and back, ending exactly where it started. Keystroke: a key that types nothing, F15 on Windows and Linux, Shift on macOS (FR-7).
- No nudge when real input happened during the interval (FR-8). The app's own nudge must not count as real input.
- Never click, scroll, type, change focus or raise a window (FR-9).
- macOS without permission and Wayland: ON with the power assertion only, and a non-blocking message; on macOS a button opens the Accessibility pane (PB-2).
- Nothing records what the user typed (NFR-2): only the OS's time since the last input.

### Impacts

- `crates/core`: `PlatformAdapter::nudge` takes the kind of nudge, and gains `idle_time`. `Capabilities` says why input is unavailable. The controller owns the schedule and keeps a bounded in-memory event log (SPEC §11). A `Ticker` thread drives it.
- `src-tauri`: the controller is shared with the ticker. New command `open_input_permission_settings`.
- `ui`: the fallback message and the macOS button. Minimal styling only, while the window design is being chosen.
- New dependency: `x11rb` on Linux (pure Rust).

### Approach

- Direct OS calls rather than `enigo`, because idle time, the permission check and Wayland detection need direct calls anyway ([ADR-0003](../../decisions/ADR-0003-direct-os-input-calls.md)).
- Skip rule: a nudge is skipped when the OS idle time is more than 1 s shorter than the time since the interval began. Our own nudge happens at the start of the interval, so only later input counts.
- Headless example: `cargo run -p unidle-core --example nudge -- once [pointer|key]` or `-- run [pointer|key|both] [interval-s] [minutes]`.

### Acceptance criteria / tests

1. Unit tests: jitter stays within ±20%, the interval is clamped, Both alternates, real input skips, our own nudge does not, unknown idle time never skips, disabled or unavailable nudges never fire, the event log is bounded.
2. X11 on a virtual display: after a pointer nudge in the middle and at each screen edge, the pointer is exactly where it started with no button pressed; after a key nudge no key is down; the idle timer resets; a Wayland session reports `Wayland` (`crates/core/tests/linux_x11_nudge.rs`).
3. SPEC acceptance criterion 4 on Windows and macOS: `nudge once pointer` leaves the pointer where it was, and `nudge once key` types nothing into a focused text editor.
4. SPEC acceptance criterion 5 on each OS: during `nudge run pointer 10 2`, moving the mouse continuously produces only "skipped" lines.
5. SPEC acceptance criterion 3 on each OS: a chat client with a 5-minute Away timeout stays Active for 30 minutes of `nudge run both 60 30`.
6. SPEC acceptance criterion 7: on macOS without the permission and on a Wayland session, the app turns ON and shows "Input simulation unavailable: keeping the system awake only."

### Out of scope

- Choosing the key from an allowlist in settings, and the method and interval settings themselves: the settings item after WI-004. The core takes a `NudgeConfig` already.
- Input on Wayland: ADR-0003's first revisit trigger.

## Execution

- Branch `feature/WI-002-input-nudge`, stacked on WI-001's branch because both change `crates/core/src/platform/`.
- Deviation: `enigo` was not adopted (ADR-0003).
- Deviation: SPEC §11 lists four adapter methods; `idle_time` is a fifth, because FR-8 needs it and the UI must still not call the OS.

## Review

1. Met. `cargo test -p unidle-core` passes 13 new unit tests in `nudge.rs` and `controller.rs`.
2. Met. `linux_x11_nudge` passes against Xvfb, including the right and bottom screen edges.
3. **Open.** Needs Windows and macOS machines.
4. **Partly met.** On X11 it follows from criterion 1's skip tests plus criterion 2's idle-timer reset; not yet run with a real mouse on any OS.
5. **Open.** Needs a real chat client.
6. **Partly met.** The Wayland branch is tested in criterion 2 and the UI message renders from the status; not yet seen on a Mac or a Wayland desktop.

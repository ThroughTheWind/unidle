---
id: WI-001
title: Prove the power assertion on all three platforms
type: spike
status: review
branch: feature/WI-001-power-assertion
created: 2026-11-03
updated: 2026-11-03
related: [WI-003, ADR-0001, ADR-0002]
epic:
children: []
---

## Proposal (rationale)

The whole product depends on holding and releasing an OS power assertion, and on the OS dropping it if the process dies (SPEC PB-3). `keepawake` is the candidate crate and hasn't been tried on any platform here. Replace `NoopAdapter` for this capability and check SPEC acceptance criteria 2 and 6 on Windows, macOS and Linux.

## Decision

`accepted` — 2026-11-03, Antoine, by asking for Phase 1 to start with this item.

## Plan

### Requirements

- While ON, the app holds an assertion that prevents idle system sleep, and idle display sleep unless "keep display on" is off (SPEC FR-6, §5.4).
- OFF, quit and process death release it (FR-10, PB-3). Releasing never panics.
- It never blocks a sleep the user asks for: lid close, Sleep from a menu (PB-4).
- It needs no permission on any OS (PB-1).

### Impacts

- `crates/core`: a new `platform` module, the only place with OS calls, and a `SystemAdapter` that replaces `NoopAdapter` in the app.
- New dependencies: `windows-sys` on Windows, `zbus` on Linux. Both are already in Tauri's tree.
- SPEC §6 names `SetThreadExecutionState`; this uses the Windows power request API instead, for the reason in ADR-0002. The spec's intent is unchanged.

### Approach

- Read `keepawake`'s source before adopting it. It panics on a failed Linux release, fails as a whole when one Linux service is missing, and is thread-bound on Windows, so the adapters call the OS directly ([ADR-0002](../../decisions/ADR-0002-direct-os-power-calls.md)).
- Linux takes every available inhibitor (ScreenSaver, GNOME SessionManager, logind `idle`) and succeeds if any is granted.
- A headless example, `cargo run -p unidle-core --example hold -- [seconds] [--display-may-sleep]`, holds the assertion so it can be inspected and killed on a real machine.

### Acceptance criteria / tests

1. `unidle-core` builds and passes clippy for Windows, macOS and Linux targets.
2. On Linux, against fake services on a private D-Bus daemon, the assertion takes every available inhibitor, falls back to the ones that exist, fails loudly when none exist, never takes a logind `sleep` lock, and releases what it took (`crates/core/tests/linux_inhibitors.rs`).
3. On each OS, `hold` shows up in the OS's own tool: `powercfg /requests`, `pmset -g assertions`, `systemd-inhibit --list` or `gnome-session-inhibit --list`.
4. SPEC acceptance criterion 2 on each OS: with idle sleep at 1 minute, `hold 1800` keeps display and system awake for 30 minutes, and after it exits the machine sleeps after 1 minute.
5. SPEC acceptance criterion 6 on each OS: killing `hold` (`taskkill /F`, `kill -9`) removes it from the tool in criterion 3.

### Out of scope

- Input nudges: WI-002.
- The "keep display on" setting in the UI and settings file: the settings item that follows WI-004. `SystemAdapter::set_power_options` is ready for it.

## Execution

- Branch `feature/WI-001-power-assertion`.
- Deviation: the plan started with `keepawake`; reading its source replaced it with direct calls (ADR-0002). The Windows API differs from SPEC §6's table for the same reason.
- The app now uses `SystemAdapter`, so turning ON in the window really keeps the machine awake.

## Review

1. Met. `cargo clippy -p unidle-core --all-targets -- -D warnings` passes for the host (Linux), `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`. Cross-checking does not link, so a wrong macOS FFI symbol name would only show on a Mac (criterion 3 catches it).
2. Met. `cargo test -p unidle-core` runs `linux_inhibitors` against a private `dbus-daemon` and passes.
3. **Open.** Needs a real Windows, macOS and Linux desktop.
4. **Open.** Same.
5. **Open.** Same.

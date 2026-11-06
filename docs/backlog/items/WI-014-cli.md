---
id: WI-014
title: Add the unidle command line, power-only by default, driving the tray app
type: feature
status: review
branch: feature/WI-014-cli
created: 2026-11-04
updated: 2026-11-04
related: [ADR-0004, WI-009, WI-010]
epic:
children: []
---

## Proposal (rationale)

Phase 4 of the roadmap starts with a CLI on the same binary. People who live in a terminal, scripts and remote sessions want `unidle on` and `unidle off`. The work-machine risk review asks that it be power-only by default and never a hidden background mode.

## Decision

`accepted` — 2026-11-04, Antoine (asked to start Phase 4; the CLI leads it).

## Plan

### Requirements

- SPEC §13, CL-1 to CL-5: `on`, `off`, `status`, `timer <length>`, `help`, `--version`; `--nudge`, `--power-only`, `--json`.
- Power-only is a property of one ON, kept in the controller and cleared by the next OFF. The latest ON decides.
- The app listens on a local channel only it and the same user can reach. A command that finds no app starts it in the tray (`on`, `timer`) or reports OFF (`off`, `status`).

### Impacts

- `crates/core`: `Controller::set_manual_with`, `start_timer_with`, `Status::power_only`; new `remote` module (protocol and transport). `libc` on Unix; Windows pipe and console features of `windows-sys`.
- `src-tauri`: `cli.rs`, `main.rs` (CLI before Tauri starts, the channel in `setup`), strings. `StatusDto` gains `powerOnly`.
- SPEC §13, ADR-0004, README.

### Approach

- See ADR-0004 for the channel and the alternatives. The CLI formats nothing about the state itself: the app answers with the tray's lines and the window's status object.

### Acceptance criteria / tests

1. Unit tests: argument parsing (commands, options, lengths, typos, the app's own flags), the request and response format, power-only in the controller, the CLI status lines.
2. A Unix socket test: no app answers `None`; a served request round-trips; the socket is `0600`.
3. On Linux (Xvfb, private D-Bus with a fake ScreenSaver): `status`/`off` with no app, `on` starting the app and taking the inhibitor, `status --json`, `on --nudge`, `timer 30m`, `timer 23:59`, `off` releasing it; after `kill -9`, the next `on` replaces the stale socket.
4. On Windows and macOS with a real build: `unidle on`, `status`, `off` from a terminal, with the tray showing ON and OFF. **Needs real machines.**

### Out of scope

- A headless mode for machines without a desktop: a new item if wanted (ADR-0004 revisit trigger 1).
- Showing power-only in the window or tray: the status object carries it; the UI change is a follow-up if wanted.
- Installing the binary on `PATH` (an installer option or a Homebrew link): with the package-manager work in Phase 5.

## Execution

- Branch `feature/WI-014-cli`, off `main`.

## Review

1. Met. `cargo test --workspace`.
2. Met. `remote::tests::serves_over_a_private_socket`.
3. Met. Run by hand in the cloud container; output in the PR.
4. **Open.** Needs real machines. The Windows code is checked with `cargo clippy --target x86_64-pc-windows-gnu` for the core and by CI.

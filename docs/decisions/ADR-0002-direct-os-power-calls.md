---
id: ADR-0002
title: Call the OS power APIs directly instead of through keepawake
status: accepted
date: 2026-11-03
supersedes:
superseded_by:
---

# ADR-0002 — Call the OS power APIs directly instead of through keepawake

## Context

[ADR-0001](ADR-0001-tauri-with-rust-core.md) named `keepawake` 0.6 as the starting candidate for the power assertion, to be proven by [WI-001](../backlog/items/WI-001-power-assertion-spike.md). Reading its source for that spike showed three problems against the spec:

- On Linux, its `Drop` calls `un_inhibit(cookie).unwrap()`. The release profile sets `panic = "abort"`, so a D-Bus error while turning OFF would crash the app.
- On Linux it fails as a whole when `org.freedesktop.ScreenSaver` is missing, and it has no GNOME `org.gnome.SessionManager` fallback, so one missing service stops the app from turning ON even when another inhibitor would work.
- On Windows it uses `SetThreadExecutionState`, whose flags belong to the calling thread. Releasing from a different thread than the one that acquired leaves the request in place, and the app turns ON and OFF from whatever thread handles the event.

Its `sleep` option would also block a sleep the user asks for, which SPEC PB-4 forbids, so part of its surface must never be used here anyway.

## Decision

The power assertion calls each OS directly from `crates/core/src/platform/`:

- **Windows:** a power request object (`PowerCreateRequest`, `PowerSetRequest` with `SystemRequired` and optionally `DisplayRequired`). Any thread can clear it, and Windows closes the handle when the process exits.
- **macOS:** IOKit `IOPMAssertionCreateWithName` with `PreventUserIdleSystemSleep` and optionally `PreventUserIdleDisplaySleep`, through a few lines of FFI.
- **Linux:** every available D-Bus inhibitor, succeeding if at least one is granted: `org.freedesktop.ScreenSaver`, `org.gnome.SessionManager`, and a logind `idle` lock, through `zbus`.

No mechanism that blocks a user-requested sleep is ever taken.

## Consequences

**Good**

- Releasing can never panic, on any OS.
- Linux works on GNOME, KDE and others, and on a machine with only logind.
- The code is about 300 lines with one new dependency per OS (`windows-sys`, `zbus`), both already in the Tauri dependency tree.

**Costs and risks**

- The project owns the FFI for three OSes. A wrong signature on macOS shows up only at link or run time on a Mac, because CI on Linux cannot check it.
- SPEC §6 lists `SetThreadExecutionState` for Windows. The power request API differs from the spec's table, though not from its intent.
- Nobody here has yet checked that each desktop drops its inhibitor when the process is killed. WI-001 asks for that check on real machines.

## Alternatives considered

**Use `keepawake` as it is** lost for the three reasons in Context, each of which breaks a spec rule (the safety property in AGENTS.md, PB-1 and PB-3).

**Fork or patch `keepawake` upstream** lost because the Windows fix means changing its API from thread-bound to handle-based, and the app would then depend on an unmerged fork for its core safety property.

**`SetThreadExecutionState` on a dedicated thread** would match the spec's table, but lost because it needs a thread kept alive for the whole session and a message channel to it, which is more code than the handle-based API for the same behavior.

## Revisit triggers

1. A real-machine check shows an OS keeps the assertion after the process is killed.
2. `keepawake` (or another maintained crate) fixes the three problems above, and dropping the FFI would remove more code than it adds.
3. A supported desktop honors none of the three Linux inhibitors.

## Admission check

Against [the rule](README.md): (1) constrains future work ✅ · (2) a real alternative was rejected
with a stated reason ✅ · (3) reversing later costs meaningfully more ✅ · (4) not owned by another
document ✅ · (5) not an implementation detail ✅.

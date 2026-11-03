---
id: ADR-0003
title: Send input nudges with direct OS calls instead of enigo
status: accepted
date: 2026-11-03
supersedes:
superseded_by:
---

# ADR-0003 — Send input nudges with direct OS calls instead of enigo

## Context

[ADR-0001](ADR-0001-tauri-with-rust-core.md) named `enigo` as the starting candidate for the input nudge, to be proven by [WI-002](../backlog/items/WI-002-input-nudge-spike.md). The nudge needs more than injecting events:

- The OS idle timer, to skip nudges while the user is active (SPEC FR-8). `enigo` does not expose it.
- The macOS Accessibility check and prompt, and Wayland detection, to fall back to the power assertion alone (SPEC PB-2). `enigo` does not expose these either.
- A guarantee that the pointer ends exactly where it started (SPEC acceptance criterion 4), which means reading the position back and restoring it, including at a screen edge where a 1 px move is clamped.

So the app needs its own OS calls on every platform whichever crate injects the events, and the events themselves are two calls per OS. [ADR-0002](ADR-0002-direct-os-power-calls.md) already made `crates/core/src/platform/` the home of direct OS calls.

## Decision

The input nudge calls each OS directly from `crates/core/src/platform/`:

- **Windows:** `SendInput` for a 1 px relative move and back, then `SetCursorPos` if acceleration left it off by a pixel; F15 down and up; `GetLastInputInfo` for idle time.
- **macOS:** CoreGraphics `CGEventPost` for an absolute move and back and a Shift flags-changed tap; `CGEventSourceSecondsSinceLastEventType` for idle time; `AXIsProcessTrusted` for the permission.
- **Linux X11:** XTEST fake input through `x11rb` (pure Rust, no C library) for an absolute move and back and F15 (or Shift if the keymap has no F15); MIT-SCREEN-SAVER for idle time.
- **Linux Wayland:** no nudge. The app reports it and keeps the power assertion.

## Consequences

**Good**

- One module per OS owns every input concern: injecting, idle time, permission, fallback.
- On Linux, `x11rb` needs no system library, so CI and packaging are unchanged.
- The X11 behavior, including the screen-edge case, is tested on a virtual display (`crates/core/tests/linux_x11_nudge.rs`).

**Costs and risks**

- More FFI owned here, and the macOS part is only proven on a Mac.
- Wayland gets no nudge at all. `enigo`'s experimental Wayland backend (libei) was not tried, and chat apps that watch input may show "Away" on Wayland.

## Alternatives considered

**`enigo`** lost because the app needs direct calls for idle time, the permission and Wayland detection anyway, and `enigo` would add a second, larger abstraction for the two calls that remain.

**`enigo` plus `user-idle` for idle time** lost for the same reason, with one more dependency, and still no permission check.

**libei / the RemoteDesktop portal on Wayland** lost for v1 because GNOME shows a permission dialog each session and support across compositors is uneven. It is the path to revisit.

## Revisit triggers

1. A Wayland input path (libei, portal) works without a per-session dialog on GNOME and KDE.
2. A real-machine check shows that a direct call here misbehaves where a maintained crate does not.

## Admission check

Against [the rule](README.md): (1) constrains future work ✅ · (2) a real alternative was rejected
with a stated reason ✅ · (3) reversing later costs meaningfully more ✅ · (4) not owned by another
document ✅ · (5) not an implementation detail ✅.

---
id: ADR-0001
title: Build the app with Tauri 2 and a separate Rust core
status: accepted
date: 2026-11-03
supersedes:
superseded_by:
---

# ADR-0001 — Build the app with Tauri 2 and a separate Rust core

## Context

The app must run on Windows, macOS and Linux with an installer under 20 MB and memory under 80 MB (SPEC NFR-1). It has to call OS power and input APIs on each platform, live in the tray, and be small enough to audit. The design system ships its tokens as CSS, and the same core may later back a CLI (SPEC §11).

## Decision

The app is a Tauri 2 shell around a UI-independent Rust crate. OS calls live only in platform adapters inside `crates/core`, using `keepawake` for power assertions and `enigo` for input as the starting candidates. The window is plain TypeScript with no UI framework.

## Consequences

**Good**

- The installer can be small because the app uses the system webview instead of bundling a browser.
- The design system's CSS tokens work as they are.
- The core can be tested and reused without a UI.
- One CI workflow builds all three platforms with `tauri-action`.

**Costs and risks**

- The system webview is the main threat to the memory limit, and it behaves differently on each OS. WI-003 measures this.
- Contributors need Rust. Linux builds also need WebKitGTK.
- `enigo`'s Wayland support is experimental, so the spec's fallback to power assertion only (SPEC PB-2) is the expected behavior there.
- The crate choices are not yet proven on all three platforms. WI-001 and WI-002 test them.

## Alternatives considered

**Electron** lost because a bundled browser puts the app at about 150 to 300 MB, far beyond the installer limit.

**Wails (Go)** has a similar size, but lost because Go isn't otherwise needed here and the Rust crates map directly onto the spec's OS calls. That comparison is a judgment and was not tested.

**Native Rust UI, such as egui** would likely use less memory, but lost because it can't reuse the design system's CSS.

## Revisit triggers

1. WI-003 shows the app can't meet the 80 MB memory limit, even with the webview closed while hidden.
2. Tauri's tray or windowing can't deliver a required behavior on a supported OS.
3. `keepawake` or `enigo` fails its spike and no alternative exists in Rust.

## Admission check

Against [the rule](README.md): (1) constrains future work ✅ · (2) a real alternative was rejected
with a stated reason ✅ · (3) reversing later costs meaningfully more ✅ · (4) not owned by another
document ✅ · (5) not an implementation detail ✅.

---
id: WI-008
title: Redesign the app and tray icons, with a Wake eye default and an App icon setting
type: feature
status: review
branch: feature/WI-008-app-icon
created: 2026-11-03
updated: 2026-11-03
related: [WI-004, WI-007]
epic:
children: []
---

## Proposal (rationale)

The app icon is still the early flat green tile with a heartbeat line, and the tray icon reuses it. The window now follows the design system (graphite neutrals, one green accent, the Halo power button), so the icon is the one surface that doesn't match. Antoine asked to "improve the icon to match the app colour", then to "also make a wake eye icon and make it so you can control it from the settings, make this one the default" (2026-11-03).

## Decision

`accepted`, 2026-11-03, Antoine.

## Plan

### Requirements

- Two app icons in the design system tokens, each the matching button lit on a graphite tile (`surface` / `bg`, dark theme, `power-on-hi` to `power-on`, `on-accent`, `power-glow` rings): the Wake eye (default, bundled) and the Halo power button.
- A setting `appIcon` (`eye` | `power`, unknown falls back to `eye`) under Settings → Appearance switches the tray, window and macOS Dock icons at once.
- It stays legible at 16, 24 and 32 px.
- The tray icons follow DESIGN_SYSTEM.md, per app icon: OFF is outlined (closed eye, or power glyph), ON is a filled green disc (open eye, or power glyph), so the shape differs and not only the color (FR-4). Colored images for Windows and Linux, template images for macOS.
- Every file listed in `tauri.conf.json` `bundle.icon` is regenerated, at the same paths and pixel sizes.

### Impacts

- `src-tauri/icons/` (all images, SVG sources), `site/favicon.svg`.
- `crates/core` settings gain `app_icon`; `src-tauri` gains `app_icon.rs` (window and Dock icon, macOS via `objc2-app-kit`), the tray picks its image pair from the setting; `ui/` gains the App icon control.
- Limit: the installer, Start menu, Finder and Linux `.desktop` icons are files the OS reads before the app runs, so they stay the Wake eye. On Wayland the taskbar also uses the `.desktop` icon.

### Approach

- Hand-written SVG sources per icon: a large one for 48 px and up and a small one (no rings, bigger disc and glyph) for 16 to 32 px. Layout in `src-tauri/icons/README.md`.
- Render with Chromium, run `tauri icon` on the 1024 px render, then replace the small PNGs and rebuild `icon.ico` so its 16, 24 and 32 px frames come from `icon-small.svg`. Steps in `src-tauri/icons/README.md`.

### Acceptance criteria / tests

- All bundle icons exist with the same sizes as before; `icon.ico` holds 16, 24, 32, 48, 64 and 256 px frames.
- A preview sheet shows the icon at every size on light and dark, and the tray icons on dark and light taskbars and the macOS menu bar.
- Settings tests: `appIcon` round-trips, defaults to `eye`, and a bad value falls back alone.
- On each OS, the installed app, taskbar / Dock and tray show the Wake eye, and switching App icon changes the tray, taskbar and Dock at once (real-machine check).

### Out of scope

- Changing the bundled installer or launcher icon at runtime (not possible from the app). Icons for Orbit and Pulse. Nothing deferred.

## Execution

Branch `feature/WI-008-app-icon`.

## Review

- Sizes and ICO frames checked with `file` and `identify`. Preview sheet rendered and reviewed.
- `cargo test -p unidle-core`, `cargo clippy --workspace --all-targets -D warnings` (Linux) and `npm run build:ui` pass. The macOS Dock code was checked with clippy for `aarch64-apple-darwin` in isolation; the full macOS build is CI's.
- Settings screen rendered with a mocked backend: the App icon control shows under Button style.
- Open: real-machine check on Windows, macOS and Linux.

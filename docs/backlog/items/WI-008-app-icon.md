---
id: WI-008
title: Redesign the app and tray icons in the design system colors
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

The app icon is still the early flat green tile with a heartbeat line, and the tray icon reuses it. The window now follows the design system (graphite neutrals, one green accent, the Halo power button), so the icon is the one surface that doesn't match. Antoine asked to "improve the icon to match the app colour" (2026-11-03).

## Decision

`accepted`, 2026-11-03, Antoine.

## Plan

### Requirements

- The app icon uses the design system tokens: a graphite tile (`surface` / `bg`, dark theme) and the Halo button ON (`power-on-hi` to `power-on`, glyph `on-accent`, `power-glow` rings).
- It stays legible at 16, 24 and 32 px.
- The tray icons follow DESIGN_SYSTEM.md: OFF is an outlined power button, ON is a filled one, so the shape differs and not only the color (FR-4). Colored images for Windows and Linux, template images for macOS.
- Every file listed in `tauri.conf.json` `bundle.icon` is regenerated, at the same paths and pixel sizes.

### Impacts

- `src-tauri/icons/` (all images, SVG sources), `site/favicon.svg`. No code change: `tray.rs` loads the same file names.

### Approach

- Hand-written SVG sources: `icon.svg` for 48 px and up, `icon-small.svg` (no rings, bigger disc and glyph) for 16 to 32 px.
- Render with Chromium, run `tauri icon` on the 1024 px render, then replace the small PNGs and rebuild `icon.ico` so its 16, 24 and 32 px frames come from `icon-small.svg`. Steps in `src-tauri/icons/README.md`.
- The Wake eye as the icon was considered; Halo is the default button style, so it is the one the icon shows.

### Acceptance criteria / tests

- All bundle icons exist with the same sizes as before; `icon.ico` holds 16, 24, 32, 48, 64 and 256 px frames.
- A preview sheet shows the icon at every size on light and dark, and the tray icons on dark and light taskbars and the macOS menu bar.
- On each OS, the installed app, taskbar / Dock and tray show the new icons (real-machine check).

### Out of scope

- A tray icon that follows the Wake eye style (DESIGN_SYSTEM.md marks it optional). Nothing deferred.

## Execution

Branch `feature/WI-008-app-icon`.

## Review

- Sizes and ICO frames checked with `file` and `identify`. Preview sheet rendered and reviewed.
- Open: real-machine check on Windows, macOS and Linux.

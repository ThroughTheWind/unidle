---
id: WI-007
title: Build the window and settings screen with the four power button styles
type: feature
status: review
branch: feature/WI-007-window-ui
created: 2026-11-03
updated: 2026-11-03
related: [WI-004, WI-005]
epic:
children: []
---

## Proposal (rationale)

The window still has the placeholder toggle from the skeleton, and every SPEC §5.4 setting can only be changed by editing `settings.json`. The design system now defines four power button styles, and Antoine chose to offer all four with Halo as the default ("keep them all as an option, with #1 as the default", 2026-11-03). F-003 also waits on this item: a failed ON says nothing.

## Decision

`accepted`, 2026-11-03, Antoine, as part of Phase 1 (SPEC FR-11, §5.4).

## Plan

### Requirements

- The main view follows `docs/design/DESIGN_SYSTEM.md`: title bar with a settings button, the power button in the chosen style, the state word ("Asleep" and "Awake" for Wake eye), the timer (inside the core for Orbit), and the reserved schedule slot.
- All four styles (`halo`, `orbit`, `pulse`, `eye`) use the markup contract in `docs/design/power-button.css`. The style is a setting, `buttonStyle`, that falls back to `halo` when missing or unknown. Changing it swaps the button at once and doesn't change ON or OFF.
- Starting and stopping show the destination look and "Starting…" or "Stopping…", with input disabled. A failure snaps back and says why in the status line (F-003).
- The status line appears only when something needs attention: an error, no power assertion, or nudges unavailable while wanted (SPEC PB-2, with the Accessibility button on macOS).
- The settings view covers every §5.4 setting: button style, simulate input, method, key (only the keys `nudge_keys` returns), interval (10 to 600 s), keep display on, launch at login, restore last state, notify on change, and a global shortcut recorder. A refused change shows the error and the control returns to the kept value.
- Geist and Geist Mono are bundled, so nothing loads from the network (NFR-2).
- All text in `ui/src/strings.ts`, and all styling from the design tokens.

### Impacts

- `crates/core`: `Settings::button_style` and `BUTTON_STYLES`.
- `ui/`: new markup, `power-button.ts` (the parts for each style), `shortcut.ts` (key press to accelerator), the settings view, and styles built on `docs/design/power-button.css`.
- `docs/design/`: the updated design-system files, plus `power-button.css` and the proposals page moved under `proposals/`.
- `package.json`: `@fontsource-variable/geist` and `@fontsource-variable/geist-mono` (OFL 1.1).

### Approach

- The OS window is the design's window frame, so the shell fills the viewport, with no radius or shadow of its own.
- Settings is a second view in the same window rather than a second window, so there's still one webview to close when hidden (WI-004).
- The window stays 360×420. The design system's layout says 440, but Antoine chose 420 (F-001), and everything fits.

### Acceptance criteria / tests

1. `npm run build:ui` and `cargo test -p unidle-core` pass, including a settings test that an unknown `buttonStyle` falls back alone.
2. In a 360×420 browser with the Tauri calls mocked, each style renders OFF and ON in light and dark, the settings view scrolls, picking a style swaps the button and keeps the state, a recorded shortcut is saved, an out-of-range interval is refused with a message, and a failed ON shows "Starting…" then snaps back with the error.
3. In the real app on Linux (Xvfb), the window opens with the stored style, toggles, and a style change is written to `settings.json`.
4. On each OS, the window looks the same as on Linux, with the bundled fonts.

### Out of scope

- The Wake eye tray icon (the design system marks it optional).
- A schedule: the slot only reserves the space (Phase 2).
- Translations.

## Execution

- Branch `feature/WI-007-window-ui`. WI-006 went to the landing page.

## Review

1. Met. `npm run build:ui`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test -p unidle-core` pass. The settings tests cover `buttonStyle` round-tripping and an unknown value falling back alone.
2. Met. Playwright at 360×420 with the Tauri calls mocked: all four styles OFF and ON in light and dark, the settings view top and bottom, a style pick that swaps the button and keeps the state, `Ctrl+Alt+K` recorded and saved, an interval of 5 refused with a message, and a failed ON that shows "Starting…" with input disabled, then snaps back with the error.
3. Met, on Xvfb with no idle inhibitor running. A `settings.json` holding `"buttonStyle": "orbit"` opened with Orbit. Clicking ON failed and the status line showed the D-Bus error, clamped to two lines (F-003). Picking Wake eye in Settings wrote `"buttonStyle": "eye"` and swapped the button.
4. **Open.** Needs real machines.

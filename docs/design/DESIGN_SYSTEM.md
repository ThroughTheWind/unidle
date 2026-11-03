# Unidle Design System

A deliberately tiny system for a single-purpose desktop utility: one window, one big ON/OFF toggle, a status line, and a tray icon. Stack-agnostic. Tokens live in [`tokens.json`](./tokens.json) (W3C Design Tokens format) and are mirrored as CSS variables in [`tokens.css`](./tokens.css). A visual reference is in [`preview.html`](./preview.html).

## Principles

1. **One glance, one action.** The state (ON or OFF) must be readable from across the room. The toggle is the hero; everything else is secondary.
2. **Native-feeling, not branded.** System fonts, quiet neutrals, follows the OS light/dark setting. It should sit politely next to Finder, Explorer or GNOME apps.
3. **Never ambiguous.** State is conveyed by position, color *and* text, never color alone.
4. **Room to grow.** Layout reserves a slot below the toggle for future scheduling, without redesigning the main view.

## Color

Semantic tokens per theme (`color.light.*`, `color.dark.*`). Components only use semantic tokens, never palette values.

| Token | Light | Dark | Use |
|---|---|---|---|
| `bg` | #F6F7F9 | #0E0F12 | Window background |
| `surface` | #FFFFFF | #14161A | Cards, panels |
| `surface-muted` | #ECEEF2 | #1B1E24 | Inputs, secondary panels |
| `border` | #D9DDE4 | #23272E | Dividers, outlines |
| `text` | #14161A | #F6F7F9 | Primary text |
| `text-muted` | #5F6672 | #9AA2AF | Secondary text, hints |
| `focus` | #2563C9 | #6AA8FF | Focus ring |
| `toggle-on-track` | #0B7A4B | #16A86A | Toggle ON |
| `toggle-off-track` | #D9DDE4 | #23272E | Toggle OFF |
| `toggle-off-border` | #5F6672 | #9AA2AF | 1.5 px OFF track outline (gives the OFF switch 3:1 contrast) |
| `on-accent` | #FFFFFF | #0E0F12 | Text/icons on the green accent (dark text in dark mode) |
| `status-active` | #0B7A4B | #5EE0A0 | "Running" dot/text |
| `status-idle` | #5F6672 | #9AA2AF | "Stopped" |
| `status-scheduled` | #2563C9 | #6AA8FF | Reserved: scheduled-but-waiting |
| `status-warning` | #9A6200 | #F5B13D | e.g. missing OS permission |
| `status-error` | #C8372B | #F27466 | Simulation failed |

Green means "keeping awake". Blue is reserved for the future scheduled state so it never collides with ON.

Checked contrast: all text tokens meet WCAG AA (4.5:1+) on `bg` and `surface` in both themes; the ON track (≥5:1) and OFF track outline (≥5:1) meet the 3:1 non-text rule; `on-accent` on the green accent is 5.4:1 (light) and 6.3:1 (dark).

## Typography

System stack (`system-ui`, Segoe UI, SF, Roboto, Ubuntu, Cantarell). No bundled fonts.

| Style | Size / weight | Use |
|---|---|---|
| `display` | 28 / 600 | State word ("On" / "Off") |
| `title` | 20 / 600 | Settings section titles |
| `label` | 15 / 500 | Control labels |
| `body` | 14 / 400 | Body, status line |
| `caption` | 12 / 400 | Hints, version, elapsed time (mono for timers) |

## Spacing, radius, elevation, motion

- **Spacing:** 4-pt scale: 4, 8, 12, 16, 24, 32, 48. Window padding is 24.
- **Radius:** `sm` 6 (inputs, chips), `md` 10 (buttons), `lg` 14 (cards), `full` (toggle, dots).
- **Shadow:** `card` for the main surface in light mode only; dark mode relies on surface contrast.
- **Motion:** 180 ms, `cubic-bezier(0.2, 0, 0, 1)` for the toggle. The ON status dot pulses (2 s). Honor `prefers-reduced-motion` / OS setting: no slide, no pulse.

## Layout

Fixed small window, 360 × 420, not resizable (or min size).

```
┌──────────────────────────────┐
│  Unidle                  ⚙   │  header: app name + settings
│                              │
│          ┌────────┐          │
│          │ ●──────│          │  hero toggle (88×48)
│          └────────┘          │
│             On               │  display text
│   ● Keeping your computer    │  status line
│     awake · 00:42:17         │  caption (mono timer)
│                              │
│ ┌──────────────────────────┐ │
│ │ Schedule (coming soon)   │ │  reserved slot: future scheduling
│ └──────────────────────────┘ │
│  v0.1.0              GitHub  │  footer caption
└──────────────────────────────┘
```

## Components

### 1. Power toggle (hero)

The only primary control. A switch, not a button: it reflects and changes state.

| Part | Spec |
|---|---|
| Track | 88 × 48, radius `full` |
| Thumb | 40 × 40, 4 px inset, `shadow.thumb` |
| Hit area | Whole track plus the state label |
| Label | `display` text under the toggle: "On" / "Off" |

**States**

| State | Track | Thumb | Label | Status line |
|---|---|---|---|---|
| Off | `toggle-off-track` + 1.5 px `toggle-off-border` | left, `toggle-off-thumb` | "Off" in `text-muted` | ○ "Stopped" (`status-idle`) |
| On | `toggle-on-track` | right, `toggle-on-thumb` | "On" in `text` | ● "Keeping your computer awake · 00:42:17" (`status-active`, pulsing dot) |
| Hover | Track 8% lighter (dark) / darker (light) | unchanged | | |
| Pressed | Thumb widens to 46 px toward travel direction | | | |
| Focus-visible | 2 px `focus` ring, 3 px offset | | | |
| Starting / stopping | Thumb at destination, track at 60% opacity, input disabled ≤ 1 s | | | "Starting…" / "Stopping…" |
| Error | Snaps back to Off | | "Off" | ▲ message in `status-error`, e.g. "Couldn't start: permission needed" |
| Disabled | 40% opacity, `not-allowed` cursor | | | Reason in caption |
| Scheduled *(future)* | `toggle-off-track` with 2 px `status-scheduled` outline | left | "Off" | ◷ "Starts at 09:00" (`status-scheduled`) |

**Accessibility:** `role="switch"`, `aria-checked`, accessible name "Simulate activity". Space/Enter toggles. Global shortcut suggestion: Ctrl/Cmd+Shift+A (owned by the specs thread).

### 2. Status line

Dot (8 px, radius `full`) + `body` text + optional mono `caption` timer. Variants: idle, active, transitioning, warning, error, scheduled. Always includes text, never dot alone.

### 3. Icon button

32 × 32 visual, 44 × 44 hit target, radius `md`, `text-muted` icon, `surface-muted` background on hover. Used for settings ⚙ and close.

### 4. Button

For settings and dialogs only (never for the main ON/OFF).

| Variant | Background | Text |
|---|---|---|
| Primary | `toggle-on-track` | `on-accent` |
| Secondary | `surface-muted` | `text` |
| Ghost | transparent | `text-muted` |

Height 36, padding 0 16, radius `md`, `label` style.

### 5. Card / section

`surface`, `border` 1 px, radius `lg`, padding 16. Holds settings groups and the future schedule panel.

### 6. Tray / menu bar icon

16 px template icon (monochrome on macOS, auto-tinted). Two glyphs:

- **Off:** outlined cursor/pulse glyph.
- **On:** same glyph filled, plus a small dot badge (green on Windows/Linux; template-only on macOS).

Tray menu: "Turn on/off" (with state checkmark), "Open window", separator, "Quit".

### 7. Reserved: schedule components (future)

Not built now, but tokens already cover them: time-range row (two `surface-muted` inputs, radius `sm`), weekday chips (32 px, radius `full`, selected = `status-scheduled` fill), and the scheduled toggle state above.

## Voice

Short, plain, present tense. "On" / "Off", "Keeping your computer awake", "Stopped". No jargon like "jiggler" or "daemon" in the UI.

## Using the tokens

- **CSS / web-based stacks (Tauri, Electron, Wails):** import `tokens.css`; theme switches via `prefers-color-scheme` or `data-theme="light|dark"` on `<html>`.
- **Native / other stacks:** read `tokens.json` directly, or generate platform outputs with [Style Dictionary](https://styledictionary.com/).

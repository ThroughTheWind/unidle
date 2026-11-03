# Unidle Design System

The design system for Unidle, a single-purpose open-source desktop app: one window and one big power button that keeps the computer awake. The finish is premium but quiet: graphite neutrals tinted slightly green, light and depth on the main control, a single green accent, and the Geist typeface.

Files in this folder:

| File | What |
|---|---|
| `tokens.json` | All tokens, W3C Design Tokens format (source of truth) |
| `tokens.css` | The same tokens as CSS variables, light/dark by `prefers-color-scheme` or `data-theme` |
| `power-button.css` | The four power button styles, the window shell and small supporting components |
| `preview.html` | Reference markup for all four styles in the window (open it next to the two CSS files) |
| `proposals/unidle-button-proposals.html` | The original exploration page |

## Principles

1. **One glance, one action.** The power button is the window. Its state reads from across the room.
2. **Premium, not loud.** Depth comes from light (a soft top-left highlight, inner shadow, one glow when on), not from many colors.
3. **Never ambiguous.** State is shown by fill, motion *and* a word ("On" / "Off"), never color alone.
4. **Personal.** The user picks one of four button styles. Halo is the default.
5. **Room to grow.** A reserved slot under the button holds future scheduling.

## Color

Token names are the same in `tokens.json` (`color.light.*`, `color.dark.*`) and `tokens.css` (`--<name>`). Dark is the hero theme; both are fully designed.

| Token | Light | Dark | Use |
|---|---|---|---|
| `bg` | #EEF1F0 | #07090A | Behind the window, page ground |
| `surface` | #FBFCFB | #111614 | Window, top of its gradient |
| `surface-sunken` | #F1F4F3 | #0B0F0E | Window, bottom of its gradient; inputs |
| `border` | #D5DBD8 | #1C2321 | Hairlines and 1px rings (decorative) |
| `text` | #0F1513 | #EEF3F1 | Primary text |
| `text-muted` | #5A6561 | #93A09B | Secondary text, state word when off |
| `focus` | #2563C9 | #6AA8FF | Focus ring |
| `power-on` | #08774F | #17B47A | Brand green: ON fill (gradient end), state word when on |
| `power-on-hi` | #14A06C | #34D697 | ON fill gradient start (top-left) |
| `power-glow` | #1FD592 | #3EF0A8 | Glow, ripples, orbit comet, window bloom. Never text |
| `power-off-face` | #E6EAE8 | #1A211F | Button face when off |
| `power-off-ink` | #5A6561 | #93A09B | Glyph on the off face |
| `on-accent` | #FFFFFF | #04110B | Glyph/text on green fills |
| `status-active` | #08774F | #5EE0A0 | Running |
| `status-idle` | #5A6561 | #93A09B | Stopped |
| `status-scheduled` | #2563C9 | #6AA8FF | Reserved for scheduling; never for ON |
| `status-warning` | #8F5C00 | #F5B13D | Needs attention (e.g. OS permission) |
| `status-error` | #C8372B | #F27466 | Failed |

Contrast (checked): every text token is 4.5:1 or better on `bg`, `surface` and `power-off-face` in both themes. `on-accent` on `power-on` is 5.0:1 (light) and 7.2:1 (dark). The glyph on `power-on-hi` is above 3:1.

## Typography

**Geist** for UI and **Geist Mono** for the timer. Both are open source (SIL OFL 1.1) and must be **bundled with the app** (woff2 or ttf from the Geist release) so it works offline and looks the same on every OS. Fallback: the system UI stack.

| Style | Size / weight / tracking | Use |
|---|---|---|
| `display` | 28 / 600 / -0.02em | Large titles |
| `title` | 20 / 600 | Settings section titles |
| `label` | 15 / 600 | Window title, labels |
| `state` | 13 / 600 / 0.16em, uppercase | "ON" / "OFF" under the button |
| `body` | 14 / 400 | Status and body text |
| `caption` | 12 / 400 | Hints, version |
| `timer` | Geist Mono 13 / 500, tabular | Elapsed time |

## Space, radius, depth, motion

- **Spacing:** 4, 8, 12, 16, 24, 32, 48. Window padding 20 × 22.
- **Radius:** `sm` 6, `md` 10, `lg` 14, `window` 22, `full`.
- **Shadows (per theme):** `shadow-window`, `shadow-face` (raised button face), `shadow-knob` (Pulse knob).
- **Motion:** `fast` 120ms, `base` 180ms, `fill` 500ms, `breathe` 3.2s, `orbit` 2.6s; easing `standard` (0.2, 0, 0, 1) and `spring` (0.3, 1.4, 0.5, 1). With reduced motion, all loops stop and fills switch instantly.

## Layout

Window 360 × 420 (SPEC FR-11, F-001), `radius-window`, `surface` → `surface-sunken` gradient, `shadow-window`. While on, a soft `power-glow` bloom sits behind the button.

```
┌──────────────────────────────┐
│ Unidle                    ⚙  │  title bar (label)
│                              │
│            ╭────╮            │
│           │  ⏻   │           │  PowerButton, chosen style
│            ╰────╯            │
│              ON              │  state word
│           00:42:17           │  timer (mono)
│ ┌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐ │
│ ╎ Schedule (coming soon)   ╎ │  reserved slot
│ └╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘ │
└──────────────────────────────┘
```

## PowerButton

The only primary control. One markup contract, four styles:

```html
<button class="ub-power" data-style="halo" role="switch" aria-checked="false"
        aria-label="Keep computer awake"> …style parts (see preview.html)… </button>
```

ON is `aria-checked="true"`. The window element gets `is-on` for the bloom.

| Style id | Name | Size | Off | On |
|---|---|---|---|---|
| `halo` | **Halo (default)** | 172 disc | Matte `power-off-face`, power glyph | Green gradient fill, slow 3.2s breathing glow, two ripple rings |
| `orbit` | Orbit | 196 ring, 152 core | Matte core in a hairline ring | A glowing comet and dot circle the ring every 2.6s; the timer moves into the core |
| `pulse` | Pulse | 236 × 104 switch | Knob left on a matte track | Knob springs right, track turns green, a heartbeat line traces through it |
| `eye` | Wake eye | 176 squircle | Closed eye, drifting z's, state word "Asleep" | Becomes a disc, eye opens, glances around, blinks every 6s, "Awake" |

States for every style: off, on, hover (slight lift), pressed (scale 0.97), focus (2px `focus` ring, 8px offset), starting/stopping (destination look, input disabled ≤1s, "Starting…"), error (snaps back to off, StatusLine shows the error), disabled (40% opacity). Space and Enter toggle.

### Button style setting

Settings has a **Button style** segmented control (`role="radiogroup"`): Halo, Orbit, Pulse, Wake eye. Store the style id (`halo`, `orbit`, `pulse`, `eye`); a missing or unknown value falls back to `halo`. Changing it swaps the button instantly without changing the on/off state.

## Supporting components

- **StatusLine:** 8px dot or glyph plus text. Variants idle, active (with timer), starting, warning, error, scheduled. Always with text.
- **Button:** 36 high, radius `md`. Primary = green gradient with `on-accent`; secondary = `power-off-face` with a `border` ring; ghost = `text-muted`. Never used for ON/OFF.
- **IconButton:** 32 visual inside a 44 hit area, `text-muted`, `power-off-face` on hover, always with `aria-label`.
- **Card:** `surface` → `surface-sunken`, 1px `border` ring, radius `lg`, padding 16.
- **Tray icon:** 16px template glyph. Off: outlined power glyph. On: filled glyph (with Wake eye selected: closed/open eye).

## Voice

Short, plain, present tense. "On" / "Off", "Keeping your computer awake", "Stopped". No jargon in the UI.

## Using the tokens

- **Web-based stacks (Tauri, Electron, Wails):** load `tokens.css`, then `power-button.css`, and register the bundled Geist fonts with `@font-face`.
- **Other stacks:** read `tokens.json`, or generate outputs with Style Dictionary.

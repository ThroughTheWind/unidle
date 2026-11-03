---
id: WI-005
title: Store settings and apply them, with launch at login and a global shortcut
type: feature
status: review
branch: feature/WI-005-settings
created: 2026-11-03
updated: 2026-11-03
related: [WI-002, WI-004]
epic:
children: []
---

## Proposal (rationale)

Every SPEC §5.4 setting is still a constant: the nudge method, key and interval, keep display on, launch at login, restore last state and the global shortcut. FR-14 and FR-15 want them in a human-readable file that never crashes the app when it is wrong. The window's settings screen waits for the design choice, so this item is the backend that screen will call.

## Decision

`accepted` — 2026-11-03, Antoine, as part of Phase 1 (SPEC §5.4 is in v1 scope).

## Plan

### Requirements

- `settings.json` in the platform config directory holds every §5.4 setting, plus the optional notification on change (FR-16), the last state, and whether the tray notice was shown (FR-14).
- A missing file, invalid JSON, or one bad value falls back to defaults for what is wrong, and keeps the rest (FR-15).
- The keystroke key comes from a closed allowlist of keys that type nothing (FR-7): F13, F15 to F20, Shift. F14 isn't offered, and F15 is refused on macOS, because macOS maps both to display brightness.
- Changing a setting applies at once: nudge schedule and key, power assertion (re-acquired while ON), launch at login, global shortcut.
- Launch at login starts in the tray without the window.
- "Restore last state" turns ON at launch when the app was ON when it last changed state. Otherwise the app starts OFF.
- The global shortcut toggles from anywhere (FR-2). A shortcut the OS refuses is reported, and the previous one stays.

### Impacts

- `crates/core`: `Settings` with a lenient reader (new dependencies `serde`, `serde_json`), and `NudgeKey`. The keystroke nudge takes the configured key.
- `src-tauri`: `settings.rs`, the commands `get_settings`, `update_settings` (any subset of fields) and `nudge_keys`, and the plugins `tauri-plugin-autostart` and `tauri-plugin-global-shortcut`. The main window is created in code (`"create": false`), so a launch at login can skip it.
- WI-004's marker file becomes the `trayNoticeShown` field. It never shipped, so no migration is needed.

### Approach

- The file is read field by field rather than with one `Deserialize`, so one bad value can't reset the others.
- `update_settings` merges the patch into the current settings and validates the result. A changed field that's invalid is an error, not a silent default.
- It writes to a temporary file and renames it, so a crash never leaves half a file.

### Acceptance criteria / tests

1. Unit tests: round trip; empty, non-JSON and non-object files give defaults; each bad value falls back alone; the interval range is enforced; a blank shortcut is unset; start ON only when restoring an ON state; the defaults match SPEC §5.4.
2. On Linux (Xvfb): a file with a bad interval and valid other fields loads without crashing, logs the fallback, and registers the shortcut. A normal launch shows the window, and `--hidden` doesn't.
3. On each OS, turning on launch at login adds the login item, the app starts in the tray at the next login, and turning it off removes the item.
4. On each OS, a global shortcut toggles with the window closed.
5. On each OS, restore last state with the app ON, then quit and relaunch, comes back ON.

### Out of scope

- The settings screen in the window: the window UI item, after the design choice.
- A settings change made by editing the file while the app runs. It's read at launch only.

## Execution

- Branch `feature/WI-005-settings`, stacked on WI-004's branch.
- The nudge key lives on the adapter (`SystemAdapter::set_nudge_key`) rather than in `NudgeConfig`, because the schedule doesn't depend on which key is pressed.
- Quitting turns OFF first (FR-13), which doesn't record OFF as the last state, so restore last state brings back the state from before the quit.

## Review

1. Met. `cargo test -p unidle-core` passes 7 new settings tests.
2. Met. Run on Xvfb with a file holding `"nudgeIntervalSecs": "soon"`, a shortcut and `keepDisplayOn: false`: it launched, logged `using defaults for nudgeIntervalSecs`, and registered the shortcut. A normal launch showed the window, and `--hidden` didn't.
3. **Open.** Needs real machines.
4. **Open.** Same.
5. **Open.** Same. The decision logic is covered by criterion 1.

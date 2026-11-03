# Unidle: Product Specification

| | |
|---|---|
| Status | Draft v0.2 |
| Date | 2026-11-03 |
| Owner | Antoine (`@ThroughTheWind`) |
| Scope | v1 (ON/OFF toggle). Scheduling is Phase 2 and described separately below. |

> Product name: **Unidle**.

---

## 1. Summary

Unidle is a small, open-source, cross-platform desktop application that keeps a computer "awake" by simulating user activity. It packages a simple wake-up routine behind a single **ON/OFF** control. When ON, the app stops the OS from going idle: no screensaver, no display sleep, no system sleep, and no "Away" status in chat apps that watch for input. When OFF, the machine behaves normally.

v1 is intentionally minimal: one toggle, sensible defaults, no account, no network access.

## 2. Goals and non-goals

### Goals (v1)
1. Start or stop activity simulation with one click or keystroke.
2. Work on **Windows, macOS and Linux** with the same behavior and UI.
3. Be invisible in day-to-day use: tiny footprint, lives in the tray / menu bar, never steals focus or disturbs the user's work.
4. Always be safe to leave running: OFF always fully restores normal OS power behavior, including after a crash.
5. Be trivially auditable: small codebase, no network, no telemetry, permissive open-source license.

### Non-goals (v1)
- Scheduling, calendars or time windows (Phase 2, §10).
- Typing real text, scripting, macros, or "human-like" behavior profiles. v1 only sends non-printing keystrokes. Richer keystroke simulation is planned for later phases (see [ROADMAP.md](ROADMAP.md)).
- Remote control, multi-device sync, cloud features, accounts.
- Hiding the app from task managers or system monitors. The app is always visible and honest about running.
- Mobile platforms.

## 3. Users and use cases

| Persona | Need |
|---|---|
| Presenter / viewer | Screen must not dim or lock during a long video, slideshow or presentation. |
| Long-running task owner | Downloads, renders, builds or remote sessions must not be interrupted by sleep. |
| Remote worker | Chat or presence status should not flip to "Away" while reading docs or on a call away from the keyboard. |
| Kiosk / dashboard operator | A display machine must stay on all day. |

**Responsible use:** the README should state that the tool simulates input only on the local machine, and that users are responsible for complying with their organization's IT and acceptable-use policies.

## 4. Core concepts

- **Simulation state:** `OFF` (default on first launch) or `ON`.
- **Activity method:** how the app keeps the machine awake. v1 combines two complementary mechanisms (§6):
  - **Power assertion:** asks the OS not to sleep (official, invisible, no permissions).
  - **Input nudge:** emits a tiny synthetic input event at an interval, so apps that watch for user input also see activity.
- **Nudge interval:** time between input nudges. Default **60 s**, with ±20% random jitter.

## 5. Functional requirements (v1)

### 5.1 ON/OFF control
- **FR-1** The app exposes one primary control: a toggle labelled with the current state ("Activity: ON" / "Activity: OFF").
- **FR-2** The toggle is reachable from:
  - the tray / menu bar icon (left-click toggles on Windows/Linux; menu item on macOS),
  - the tray menu ("Turn ON" / "Turn OFF"),
  - the main window's large toggle button,
  - an optional global keyboard shortcut (default unset; configurable in settings).
- **FR-3** State changes take effect within **1 second**.
- **FR-4** The tray icon visibly differs between ON and OFF (shape, not color alone), and its tooltip reads "Unidle: ON" or "Unidle: OFF".
- **FR-5** While ON, the main window and tray menu show elapsed time since it was turned on ("ON for 1 h 12 min").

### 5.2 Activity simulation
- **FR-6** When ON, the app holds an OS power assertion that prevents idle system sleep and display sleep (§6).
- **FR-7** When ON and the input nudge is enabled (default: enabled), the app emits one synthetic input event per nudge interval. The event must have **zero net visible effect**:
  - **Pointer** (default): move the pointer by 1 px and back immediately, so the pointer ends exactly where it started.
  - **Keystroke**: press and release a non-printing key with no default action (F15 on Windows/Linux; a no-op modifier tap such as Shift on macOS). Configurable key, from a safe allowlist.
  - **Both**: alternate pointer and keystroke nudges. Some presence trackers only count keyboard input, others only pointer input.
- **FR-8** The app skips a nudge if real user input happened within the current interval, so it never interferes with someone who is actively working.
- **FR-9** The nudge must never click, scroll, type characters, change focus, or bring any window to the front.
- **FR-10** When turned OFF (or when the app quits), the power assertion is released and nudges stop immediately.

### 5.3 Main window
- **FR-11** A single small window (about 320×240) containing: app name, the large toggle, status line (state and elapsed time), and a link to Settings.
- **FR-12** Closing the window hides it to the tray; it does not quit the app. The first time this happens, show a one-time notice ("Still running in the tray").
- **FR-13** "Quit" lives in the tray menu and the app menu. Quitting while ON turns simulation OFF first.

### 5.4 Settings (minimal)
| Setting | Default | Notes |
|---|---|---|
| Input nudge enabled | On | Off means power assertion only. |
| Nudge method | Pointer 1 px | Pointer, Keystroke or Both. |
| Nudge interval | 60 s | Range 10 s to 10 min. |
| Keep display on | On | Off allows the screen to turn off while the system stays awake. |
| Launch at login | Off | Uses the platform's standard autostart mechanism. |
| Restore last state on launch | Off | When off, the app always starts OFF. |
| Global shortcut | Unset | e.g. `Ctrl+Alt+A` / `⌃⌥A`. |

- **FR-14** Settings are stored locally in a human-readable file (JSON or TOML) in the platform's standard config directory.
- **FR-15** Invalid or missing settings fall back to defaults without crashing.

### 5.5 Notifications
- **FR-16** No notifications by default. Optional OS notification on state change (setting, default off).

## 6. Platform behavior

| Platform | Power assertion | Input nudge | Permissions |
|---|---|---|---|
| **Windows 10/11** | `SetThreadExecutionState(ES_CONTINUOUS \| ES_SYSTEM_REQUIRED \| ES_DISPLAY_REQUIRED)` | `SendInput` | None |
| **macOS 12+** | IOKit `IOPMAssertionCreateWithName` (`PreventUserIdleDisplaySleep` / `PreventUserIdleSystemSleep`) | CoreGraphics `CGEventPost` | **Accessibility** permission required for the input nudge only. |
| **Linux (X11)** | `org.freedesktop.ScreenSaver.Inhibit` / systemd-logind inhibitor over D-Bus | XTest | None |
| **Linux (Wayland)** | Same D-Bus inhibitors (GNOME, KDE) | Not generally possible. Fall back to power assertion only. | None |

Rules:
- **PB-1** The power assertion works without any special permission on every platform and is always used when ON.
- **PB-2** If the input nudge is unavailable (macOS permission denied, Wayland), the app still turns ON using the power assertion only and shows a clear, non-blocking message: "Input simulation unavailable: keeping the system awake only." On macOS it includes a button that opens the Accessibility settings pane.
- **PB-3** OS-held assertions are released automatically if the process dies, so a crash never leaves the machine unable to sleep. This must be verified on each platform.
- **PB-4** The app does not override explicit user actions: closing a laptop lid, choosing "Sleep" manually, or locking the screen still work normally.

## 7. Non-functional requirements

- **NFR-1 Footprint:** idle CPU under 0.5%, memory under 80 MB (target under 30 MB), installer under 20 MB.
- **NFR-2 Privacy:** no network access at all in v1 (no telemetry, no update checks without explicit opt-in). No logging of user input.
- **NFR-3 Reliability:** runs for 7+ days ON without memory growth or missed nudges.
- **NFR-4 Accessibility:** full keyboard operation of the window, screen-reader labels on the toggle and status, state never conveyed by color alone, respects OS dark/light mode and reduced motion.
- **NFR-5 Localization-ready:** all UI strings externalized. v1 ships English; French is a candidate second language.
- **NFR-6 Startup:** visible in the tray in under 2 seconds.

## 8. Distribution and open source

- **License:** MIT.
- **Repository:** public on GitHub at [`ThroughTheWind/unidle`](https://github.com/ThroughTheWind/unidle), with README, LICENSE, CONTRIBUTING, CODE_OF_CONDUCT, issue and PR templates, and this spec under `docs/`.
- **Builds:** CI produces installers for each platform on tagged releases, attached to GitHub Releases:
  - Windows: `.msi` or `.exe` installer (plus portable `.zip` if cheap).
  - macOS: universal `.dmg` (Apple Silicon and Intel). Signed and notarized when a developer ID is available; until then, document the Gatekeeper bypass.
  - Linux: `.AppImage` and `.deb` at minimum.
- **Versioning:** SemVer, with a CHANGELOG.
- **Development tooling:** the `rungs` agentic dev system (`@rungs/cli`) is installed into the repo. It is development tooling only and is not shipped in the app.
- **Stack ([ADR-0001](decisions/ADR-0001-tauri-with-rust-core.md)):** Tauri 2 with a Rust core. The simulation core and platform adapters (§11) are a Rust crate, and the UI shell is the Tauri webview. Final repo layout is defined by the stack/repo-setup work. UI follows the project design system.

## 9. Acceptance criteria (v1)

1. On a fresh install on each platform, the app launches OFF with a tray icon.
2. With OS idle sleep set to 1 minute, turning the app ON keeps the display and system awake for at least 30 minutes; turning it OFF lets the machine sleep again after 1 minute.
3. With the input nudge on (X11, Windows, macOS with permission), a presence-tracking app (e.g. a chat client with a 5-minute "Away" timeout) stays "Active" for 30 minutes without real input.
4. After each nudge, the pointer position is identical to before the nudge, and no click, keystroke, focus change or scroll was produced.
5. While the user types or moves the mouse continuously, no nudges are emitted.
6. Killing the process while ON (`kill -9` / Task Manager) restores normal sleep behavior.
7. On macOS without Accessibility permission, and on Wayland, the app turns ON in power-assertion-only mode and shows the explanatory message.
8. Closing the window keeps the app running in the tray; Quit from the tray exits and releases everything.
9. The app makes zero outbound network connections (verified with a network monitor).

## 10. Phase 2: Scheduling (future)

Not in v1, but v1 should not block it. The full phased plan and the feature candidates are in [ROADMAP.md](ROADMAP.md). The settings model and the "simulation controller" should treat the toggle as one input among several so a scheduler can drive it later.

Planned capabilities:
- **Schedules:** one or more weekly time windows (e.g. Mon to Fri, 09:00 to 18:00) during which simulation is automatically ON.
- **Timers:** "ON for 30 min / 1 h / 2 h / until 18:00" quick actions from the tray menu.
- **Mode indicator:** state shows its source: Manual, Scheduled or Timer ("ON, scheduled until 18:00").
- **Precedence rules (proposed):** a manual toggle always wins until the next schedule boundary; timers override schedules; turning OFF manually during a scheduled window pauses it until the window ends.
- **Conditions (candidate):** only when on AC power; only when a given app is running; stop when the screen is locked.
- **Time handling:** local time zone, correct across DST changes and sleep/resume.

## 11. Architecture constraints (stack-agnostic)

These constraints apply whatever stack is chosen:
- Separate the **simulation core** (platform adapters for power assertion and input nudge) from the **UI shell**, so the core is unit-testable and could also ship as a CLI (`unidle on|off|status`).
- One **controller** owns the state machine: `OFF → ON(source) → OFF`, with an event log kept in memory only.
- Platform adapters expose a common interface: `acquire()`, `release()`, `nudge()`, `capabilities()`.
- The UI only talks to the controller, never to OS APIs directly.

## 12. Open questions

1. ~~**Product name.**~~ Resolved: **Unidle**.
2. ~~**Relationship to "rungs".**~~ Resolved: `rungs` (`@rungs/cli`) is a dev-tooling CLI installed into the repo. It is not a runtime dependency (see §8).
3. **Default nudge method.** Pointer, keystroke or both?
4. **Global shortcut default.** Ship unset, or with a default?
5. **Code signing.** Is budget available for an Apple Developer ID and a Windows code-signing certificate?
6. ~~**License.**~~ Resolved: MIT.

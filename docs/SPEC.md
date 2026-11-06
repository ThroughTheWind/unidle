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
- Scheduling, calendars or time windows (Phase 2, §10; calendars are Phase 4).
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
- **FR-11** A single small window (360×420, per the design system) containing: app name, the large toggle, status line (state and elapsed time), a link to Settings, and a "Check for new versions" icon button. The Settings screen adds a "Website" icon button. Both open the page in the default browser; the app itself makes no request (NFR-2).
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

- **NFR-1 Footprint:** idle CPU under 0.5%, installer under 20 MB, and memory under 80 MB (target under 30 MB) while hidden in the tray. The memory limit doesn't apply while the window is open, so closing the window must release the webview.
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

## 10. Phase 2: Scheduling

Phase 2 of the [roadmap](ROADMAP.md). The toggle is one input among several: the user, a timer and a weekly schedule each say whether simulation should be ON, and the controller decides between them.

- **SC-1 Schedules:** one or more weekly time windows (e.g. Mon to Fri, 09:00 to 18:00) during which simulation is automatically ON. Times are local wall-clock times. A window whose end is at or before its start runs overnight into the next day. Overlapping windows merge.
- **SC-2 Skip days:** dates on which the schedule doesn't run (holidays). A window that starts on a skip day is skipped.
- **SC-3 Timers:** "ON for 15 min / 30 min / 1 h / 2 h / until HH:MM" quick actions from the tray menu and the window. "Until" a time already past today means tomorrow.
- **SC-4 Source indicator:** the state shows its source, Manual, Timer, Scheduled or Meeting (§18), and the next change ("Timer: OFF at 18:00", "Scheduled: ON at 09:00").
- **SC-5 Precedence:**
  - The most recent manual action or timer wins over the schedule. A manual action cancels a running timer, and a new timer replaces a manual action.
  - A manual ON or OFF that differs from the schedule holds until the schedule next changes. Turning OFF during a window pauses it until the window ends. Turning ON outside one keeps it ON until the next window starts, and the schedule takes over from there. A manual action that matches the schedule (ON again during a paused window) resumes following it.
  - A timer keeps simulation ON until it ends. Then the schedule decides: still ON inside a window, OFF otherwise.
  - Without an enabled schedule, a manual change holds until the next one, as in v1.
- **SC-6 Time handling:** the state is computed from the wall clock and the system time zone each time it is checked, at least every 15 seconds while a timer or schedule runs. A DST change never moves a window's local times, and after a sleep or resume the app lands in the right state within 15 seconds without replaying what it missed.
- **SC-7 Quitting** always turns OFF, whatever a timer or schedule wants. Timers are not kept across a restart; the schedule is.
- **Conditions (Phase 4):** see §14.

## 11. Architecture constraints (stack-agnostic)

These constraints apply whatever stack is chosen:
- Separate the **simulation core** (platform adapters for power assertion and input nudge) from the **UI shell**, so the core is unit-testable and could also ship as a CLI (`unidle on|off|status`, §13).
- One **controller** owns the state machine: `OFF → ON(source) → OFF`, with an event log kept in memory only. The activity log (§15) is separate: opt-in, one row per session, never the event log.
- Platform adapters expose a common interface: `acquire()`, `release()`, `nudge()`, `capabilities()`.
- The UI only talks to the controller, never to OS APIs directly.

## 12. Open questions

1. ~~**Product name.**~~ Resolved: **Unidle**.
2. ~~**Relationship to "rungs".**~~ Resolved: `rungs` (`@rungs/cli`) is a dev-tooling CLI installed into the repo. It is not a runtime dependency (see §8).
3. **Default nudge method.** Pointer, keystroke or both?
4. **Global shortcut default.** Ship unset, or with a default?
5. **Code signing.** Is budget available for an Apple Developer ID and a Windows code-signing certificate?
6. ~~**License.**~~ Resolved: MIT.

## 13. Phase 4: Command line

Phase 4 of the [roadmap](ROADMAP.md). The same `unidle` binary takes commands from a terminal, for scripts, remote sessions and people who prefer one. It drives the tray app; it is not a second, hidden way to run Unidle ([ADR-0004](decisions/ADR-0004-cli-drives-the-tray-app.md)).

| Command | Effect |
|---|---|
| `unidle on` | ON, manual, until `unidle off` or any other OFF |
| `unidle off` | OFF. Pauses a schedule window, like the toggle (SC-5) |
| `unidle status` | The state, its source and next change, and whether nudges are sent |
| `unidle timer <length>` | A timer (SC-3): `30m`, `1h`, `1h30m`, `45` (minutes) up to 24 h, or a time such as `18:00` |
| `unidle help`, `--version` | Usage and version |

- **CL-1 Power only by default.** `on` and `timer` hold the power assertion without input nudges, whatever the settings say, until the next OFF. `--nudge` asks for the nudges from Settings instead; `--power-only` says the default out loud. The latest ON decides: the window's toggle or the tray turning ON again follows the settings. Power-only covers presentations, renders and remote sessions without simulated input, the part that draws scrutiny on managed machines.
- **CL-2 Always visible.** A command only reaches a running app. When `on` or `timer` finds none, it starts the app in the tray first, so every ON shows in the tray and quitting the app turns it OFF (SC-7). There is no daemon or background mode without the tray. `off` and `status` never start the app: not running is OFF.
- **CL-3 Output.** Text by default, the same lines as the tray; `--json` prints the window's status object. Exit status 0 on success, 1 when the app refuses or can't be reached, 2 for a usage error.
- **CL-4 Local only.** Commands travel over a Unix socket in a folder private to the user (`$XDG_RUNTIME_DIR`, else a `0700` folder in the temp folder), or a named pipe that refuses remote clients on Windows. No network (NFR-2), and the app ignores anything that isn't one short request.
- **CL-5 Arguments the app doesn't know as a command** (such as `--hidden` from the login item) start the app as before. An unknown word is a usage error, so a typo never opens a window.
- **CL-6 Links.** The installers register the `unidle:` scheme: `unidle://on`, `unidle://off`, `unidle://timer/30m` and `unidle://timer/18:00` do what the commands do, for Apple Shortcuts ("Open URLs"), launchers and scripts. A link can come from a web page, so links are always power-only (no `--nudge`), never answer with the status, and need the OS's or browser's own prompt before they open. Windows and Linux start the binary with the link, which reaches the app like a command (CL-2); macOS hands it to the app.
- **CL-7 PowerShell.** A module in `integrations/powershell` wraps the binary: `Enable-Unidle [-Nudge]`, `Disable-Unidle`, `Start-UnidleTimer -Duration | -Until [-Nudge]`, `Get-UnidleStatus`. Each returns the status object.

## 14. Phase 4: Conditions

Phase 4 of the [roadmap](ROADMAP.md). Conditions decide whether Unidle should run right now. When the user, a timer or the schedule wants it ON but a condition says no, it is **held**: the power assertion is released and nudges stop, the window and tray say "Paused" and why, and it resumes on its own when the condition clears. Clicking the toggle while held turns it OFF.

| Setting | Default | Effect |
|---|---|---|
| Only when plugged in | Off | Held on battery power |
| Pause on low battery | Never | Held on battery below 10, 20, 30 or 50 % |
| Only while an app is running | Empty | Held unless one of the named processes runs (case and `.exe` ignored) |
| Only on these Wi-Fi networks | Empty | Held unless joined to one of the named networks (case ignored); on a cable with no Wi-Fi, held |
| Only while a VPN is connected | Off | Held while no VPN tunnel is up |
| Let it sleep while locked | Off | Held while the screen is locked |
| Turn off after sleep | On | After the computer sleeps while ON or held, it is OFF |

- **CO-1 Never on a guess.** A reading the OS can't give (no battery information, the lock state unknown) never holds anything.
- **CO-2 No nudges on a locked screen**, whatever the settings: the power assertion stays, so a render keeps running, but nothing is simulated for nobody.
- **CO-3 Sleep.** While ON the assertion blocks idle sleep, so time asleep means the user chose Sleep or closed the lid (PB-4). On waking, Unidle turns OFF as a manual OFF would: a timer is cancelled and a schedule window pauses until it ends (SC-5). Detected by comparing a clock that counts sleep with one that doesn't (`CLOCK_BOOTTIME`/`CLOCK_MONOTONIC`, `QueryInterruptTime`/`QueryUnbiasedInterruptTime`, `mach_continuous_time`/`mach_absolute_time`), within 15 s of waking.
- **CO-4 Polling.** Conditions are read every 5 s while ON or held, only those that are set, and never while OFF. Lock: logind `LockedHint` or `org.freedesktop.ScreenSaver.GetActive` on Linux, the input desktop on Windows, `CGSSessionScreenIsLocked` on macOS. Power: `/sys/class/power_supply`, `GetSystemPowerStatus`, IOKit power sources. Apps: `/proc`, a ToolHelp snapshot, `proc_listallpids`. Only process names are read, never their contents or windows (NFR-2). Wi-Fi: NetworkManager's active access point, the WLAN API's current connection, `ipconfig getsummary`; Windows 11 24H2 and recent macOS hide the network name from apps without Location access, and then this condition never holds (CO-1). VPN: an interface that is up with an IPv4 address and is a tunnel by name (`tun`, `tap`, `wg`, `ppp`, `ipsec`, `utun`) or by type (Linux `ARPHRD_NONE`), or on Windows an adapter that is up and is PPP, a virtual adapter, or describes itself as a VPN, WireGuard or TAP tunnel. No traffic is sent or inspected.
- **CO-5 Precedence.** Conditions come after SC-5: they never turn anything ON, only hold what the user, a timer or the schedule turned ON.

## 15. Phase 4: Activity log

Phase 4 of the [roadmap](ROADMAP.md). An opt-in, local record of when Unidle kept the computer awake, for the user's own timesheets or for showing IT what it did.

- **AL-1 Opt-in.** Off by default. Settings → Activity log → "Keep an activity log". Turning it off stops adding rows and keeps the ones already there.
- **AL-2 One row per ON session**, written when it ends: turned OFF, paused by a condition (§14), the computer slept (CO-3, ending when it fell asleep, not when it woke), or the app quit. A session cut short by a crash, or by the process being killed, has no row. A timer that ends inside a schedule window continues the same session.
- **AL-3 Columns:** `start,end,duration_seconds,source,power_only,nudges`. Times are local with their UTC offset (`2026-11-05T09:00:00+01:00`); `source` is `manual`, `timer`, `schedule` or `calendar`, the one it started with; `power_only` is `true` when any of it ran without nudges by request (§13); `nudges` counts the ones sent. Nothing about what the user did, typed or ran is written (NFR-2).
- **AL-4 Stays on this computer**: `activity-log.csv` in the app's data directory, at most 5,000 rows, the oldest dropped first. "Export…" saves a copy where the user picks; "Clear" (clicked twice) deletes it.

## 16. Phase 4: Work computers

Phase 4 of the [roadmap](ROADMAP.md). On a computer an organization manages, its IT team can likely see Unidle, and its rules decide whether Unidle may be used there. Unidle says so once, and obeys a policy IT sets.

- **WC-1 One check, at the first launch that shows the window**, and again from Settings → System → Work computer → Check. It is local and read-only: Windows, joined to an Active Directory domain (`NetGetJoinInformation`) or Entra ID (`HKLM\SYSTEM\CurrentControlSet\Control\CloudDomainJoin\JoinInfo`), or enrolled in MDM (an `HKLM\SOFTWARE\Microsoft\Enrollments` entry with a `ProviderID`); macOS, `profiles status -type enrollment` saying Yes, or an Active Directory binding; Linux, `/etc/sssd/sssd.conf` or `/etc/krb5.keytab`. It never looks for monitoring, security or tracking products by name.
- **WC-2 The notice**, when a signal is found: "This computer appears to be managed by an organization. Its IT team can likely see the apps you install and may monitor activity. Check your organization's policy before using Unidle here." with **Use power-only mode** (turns Simulate input off), **Continue**, and **Learn more** (the README's "On a work computer"). It never blocks.
- **WC-3 No reassurance.** Finding nothing shows nothing at launch; from Settings it says "No sign found. A work computer can still have its own rules." There is never a "safe" verdict or score.
- **WC-4 IT policy.** `DisableInputSimulation` turns input nudges off whatever the settings say, read once at launch: a `DWORD` of 1 under `HKLM` or `HKCU\SOFTWARE\Policies\Unidle` on Windows, a managed (forced) preference for `com.throughthewind.unidle` on macOS, `{"disableInputSimulation": true}` in `/etc/unidle/policy.json` on Linux. The power assertion still works. The window says "Your organization turned off input simulation", and `unidle status` says "Input nudges: off by your organization's policy". To block Unidle entirely, IT uses its application control (AppLocker, WDAC, Santa), which does that better than an app could.

## 17. Phase 4: Scripts

Phase 4 of the [roadmap](ROADMAP.md). A command the user writes runs when Unidle turns ON or OFF, for example to set a chat status, pause a sync, or log to their own tool.

- **SR-1 Opt-in.** Settings → Scripts, "When it turns on" and "When it turns off". Empty, the default, runs nothing.
- **SR-2 When.** ON is when the power assertion is taken, whatever turned it on; OFF is when it's let go: turned off, paused by a condition (§14), after sleep, or quitting. Turning ON while already ON runs nothing.
- **SR-3 How.** With the user's shell (`/bin/sh -c`, or `cmd /C` with no window on Windows), as the user, with `UNIDLE_EVENT` (`on` or `off`) and `UNIDLE_SOURCE` (`manual`, `timer`, `schedule` or `calendar` on ON). Unidle doesn't wait for it, show its output, or stop it; a failure is only logged.
- **SR-4 Local.** Unidle runs exactly the line in the settings file, which only the user and the window can change; nothing else can set it, and the app adds no network of its own (NFR-2).

## 18. Phase 4: Calendar

Phase 4 of the [roadmap](ROADMAP.md). Unidle turns ON during the meetings in a calendar file, for example so the screen stays on through a call. The file is local: Unidle never fetches a calendar (NFR-2). To follow an online calendar, export or sync it to a file with the calendar app.

- **CA-1 Opt-in.** Settings → Calendar → Choose… picks an iCalendar (`.ics`) file; Remove stops following it. No file, the default, plans nothing.
- **CA-2 Meetings.** An event turns it ON from its start to its end, like a schedule window (§10): the source is Meeting (`calendar`), a manual OFF pauses the meeting until it ends ("Meeting paused until 15:00"), and a manual ON or a timer wins over it. When a schedule window and a meeting overlap, the window names the source and it stays ON until both end.
- **CA-3 What counts.** Events with a start time and an end or a duration. All-day events, events marked free (`TRANSP:TRANSPARENT`) and cancelled ones don't count. Recurring events follow `RRULE` (daily, weekly, monthly and yearly, with `INTERVAL`, `COUNT`, `UNTIL`, `BYDAY` and `BYMONTHDAY`), `EXDATE` and moved occurrences (`RECURRENCE-ID`); a rule with parts beyond these keeps only its first meeting. Times follow their `TZID` when it names an IANA zone, and the computer's own zone otherwise. Nothing the reader can't make sense of turns it ON.
- **CA-4 Kept current.** The file is checked every minute and read again when it changes; the meetings from a day back to two weeks ahead are worked out again every hour. A file that can't be read plans nothing, and Settings says why.
- **CA-5 Private.** Only the times are kept, in memory. Titles, places and attendees are skipped and never stored, logged or shown.

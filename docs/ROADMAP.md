# Unidle: Development Roadmap

| | |
|---|---|
| Status | Draft v0.2 |
| Date | 2026-11-04 |
| Companion to | [SPEC.md](SPEC.md) |

Each phase ships a usable, released version. Phases are ordered by value and risk, not by calendar dates. Size estimates are rough, for one part-time developer: **S** is under a week, **M** is 1 to 3 weeks, **L** is more than 3 weeks.

---

## Phase overview

| Phase | Version | Theme | Outcome |
|---|---|---|---|
| 0 | — | Foundations | Repo, CI, release pipeline, platform adapter proof of concept |
| 1 | 1.0 | **ON/OFF MVP** | Tray app with one toggle, power assertion, pointer and keystroke nudges |
| 2 | 1.x | **Timers and scheduling** | Quick timers, weekly schedules, automatic ON/OFF |
| 3 | — | ~~Rich keystroke and activity simulation~~ | **Removed** (2026-11-04). See below. |
| 4 | 2.0 | **Smart conditions and integrations** | CLI, context rules (power, apps, network, calendar), automation hooks |
| 5 | 3.0 | **Polish and ecosystem** | Auto-update, localization, plugins, signed stores |

---

## Phase 0: Foundations

**Goal:** prove the risky platform parts before building any UI.

| Item | Size | Notes |
|---|---|---|
| Repo setup (license, README, CONTRIBUTING, templates, `docs/`) | S | Run by the stack/repo-setup work. |
| CI: lint, test, build on Windows, macOS and Linux | S | GitHub Actions matrix. |
| Release pipeline producing unsigned installers on tag | M | `.msi`/`.exe`, `.dmg`, `.AppImage`/`.deb`. |
| Spike: power assertion on 3 OSes | S | Confirm automatic release when the process is killed. |
| Spike: pointer and keystroke injection on 3 OSes | M | Includes the macOS Accessibility prompt and Wayland detection. |
| Core / adapter interfaces (`acquire`, `release`, `nudge`, `capabilities`) | S | See SPEC §11. |

**Exit criteria:** a headless test binary turns ON/OFF on each OS and passes SPEC acceptance criteria 2, 4 and 6.

## Phase 1: ON/OFF MVP (v1.0)

**Goal:** everything in [SPEC.md](SPEC.md) §5 to §9.

- Tray / menu bar icon with ON/OFF and elapsed time.
- Small main window with the large toggle, styled with the design system.
- Power assertion, with a "keep display on" option.
- Input nudge: **pointer**, **keystroke** (non-printing key from a safe allowlist) or **both**, with interval and jitter.
- Skip nudges while the user is active.
- Graceful fallback (macOS without permission, Wayland).
- Settings file, launch at login, optional global shortcut.
- Zero network access.

**Exit criteria:** all SPEC §9 acceptance criteria pass on Windows 11, macOS 14+ and Ubuntu LTS (X11 and Wayland). Public GitHub release.

## Phase 2: Timers and scheduling (v1.x)

**Goal:** the app turns itself on and off.

| Feature | Size | Priority |
|---|---|---|
| Quick timers in the tray: 15 min, 30 min, 1 h, 2 h, "until HH:MM" | S | Must |
| Weekly schedules (several windows per day, per weekday) | M | Must |
| State source indicator: Manual, Timer or Scheduled | S | Must |
| Precedence rules: manual > timer > schedule (SPEC §10) | S | Must |
| Correct behavior across DST changes and sleep/resume | S | Must |
| Holiday / skip-day exceptions | S | Should |
| "Next change" preview in the tray ("OFF at 18:00") | S | Should |
| Import/export schedules as a file | S | Could |

## Phase 3: removed

Phase 3 planned richer keyboard and activity simulation: custom key combos and sequences, human-like typing into a scratch pad or a chosen window, pointer paths, scroll nudges and activity profiles. **It was dropped on 2026-11-04 and will not be built**, in the core or as a plugin.

Why: simulated typing and "realistic" activity are built to make a person look present to monitoring tools, which is the use most likely to get users disciplined on work machines and to get Unidle described as an evasion tool. The Phase 1 nudges (a 1 px pointer move and one non-printing key) stay as they are. The phase number is kept so that "Phase 4" and "Phase 5" keep meaning what they meant.

## Phase 4: Smart conditions and integrations (v2.0)

**Goal:** the app decides on its own when activity makes sense.

| Feature | Size | Priority |
|---|---|---|
| Conditions: only on AC power; stop below N% battery | S | Should |
| Conditions: only while a given app or process is running (e.g. a video call app, a render) | M | Should |
| Conditions: only on a given Wi-Fi network or when a VPN is connected ([SPEC §14](SPEC.md#14-phase-4-conditions)) | M | Could |
| Stop when the screen is locked or the user chooses Sleep | S | Must |
| Calendar awareness: ON during meetings from a local `.ics` file, with no network ([SPEC §18](SPEC.md#18-phase-4-calendar)) | M | Could |
| CLI on the same binary: `unidle on|off|status|timer 30m`, power-only unless asked otherwise, always visible in the tray (no hidden background mode) ([SPEC §13](SPEC.md#13-phase-4-command-line)) | S | Must |
| Local automation hooks: URL scheme (`unidle://on`), Apple Shortcuts, Windows PowerShell module ([SPEC §13](SPEC.md#13-phase-4-command-line)) | M | Could |
| Run-on-event scripts: user commands run on ON/OFF (opt-in, local only) ([SPEC §17](SPEC.md#17-phase-4-scripts)) | S | Could |
| Activity log: local history of sessions (start, stop, source, duration) with CSV export ([SPEC §15](SPEC.md#15-phase-4-activity-log)) | S | Should |
| Managed-device notice: a one-time, read-only local check (domain or MDM enrollment) that suggests power-only mode, never a "safe" score ([SPEC §16](SPEC.md#16-phase-4-work-computers)) | S | Should |
| Admin policy: an IT-set registry key or managed preference can turn off input nudges, and the app says so ([SPEC §16](SPEC.md#16-phase-4-work-computers)) | S | Could |

## Phase 5: Polish and ecosystem (v3.0)

| Feature | Size | Priority |
|---|---|---|
| Opt-in auto-update (signed release feed). Options and spec changes: [update-check study](studies/update-check.md) | M | Should |
| Code signing and notarization on all platforms | M | Should (depends on budget) |
| Package managers: winget, Homebrew cask, Flathub, AUR, Snap | M | Should |
| Localization: French first, then community translations | S | Should |
| Themes and icon packs following the design system | S | Could |
| Multi-monitor and multi-seat awareness | M | Could |
| Optional companion: remote toggle from phone over the local network | L | Won't (for now) |

---

## Candidate feature backlog (unranked)

Ideas worth keeping that don't fit a phase yet:
- "Presentation mode": auto ON when an app is fullscreen.
- Download/upload watcher: stay ON while network throughput is above a threshold, then turn OFF.
- CPU watcher: stay ON while a build or render keeps CPU busy.
- "Keep chat app active" presets tuned per app (Slack, Teams, Discord).
- Daily usage summary in the tray ("ON for 3 h 20 min today").
- Portable mode (settings next to the executable, no install).
- Headless/server mode for Linux machines without a desktop.
- Accessibility profile: very large toggle and high-contrast tray icons.

## Cross-cutting principles (all phases)

1. **Never harm the user's work.** No clicks, no focus changes, no printable characters, ever.
2. **Always visible and honest.** The app never hides from the OS, the tray or task managers.
3. **Local first.** No network in the core. Any networked feature is opt-in and documented.
4. **OFF is always safe.** Turning OFF, quitting or crashing restores normal OS behavior.
5. **Small and auditable.** Every new feature has to justify its size.

## Open questions

1. ~~Should printable typing simulation ship at all, or stay a plugin outside the core app?~~ Resolved: no. Phase 3 is removed.
2. ~~Is calendar integration worth adding network access for, even as opt-in?~~ No (2026-11-06): calendar awareness reads a local `.ics` file only (SPEC §18).
3. Which package managers matter most to you for the first release?

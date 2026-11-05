# Unidle

A small, open-source desktop app for Windows, macOS and Linux that keeps your computer awake with one ON/OFF toggle.

> **Status: 0.2.0.** Turning ON keeps the computer awake and sends input nudges, quick timers and a weekly schedule turn it on and off for you, the app lives in the tray, and every setting is in the window. Download it from the [landing page](https://throughthewind.github.io/unidle/) or [GitHub Releases](https://github.com/ThroughTheWind/unidle/releases). What changed is in the [changelog](CHANGELOG.md), and what comes next in the [roadmap](docs/ROADMAP.md).

## What it does

When ON, the app asks the OS not to sleep and sends a tiny input event at an interval, with no visible effect, so apps that watch for input see activity too. When OFF, or if the app crashes, your computer behaves normally. It makes no network connections. The full behavior is in the [spec](docs/SPEC.md).

If you turn on Settings → Activity log, it keeps a CSV of when it was ON, why, and how many nudges it sent, on your computer only, to export when you need it ([SPEC §15](docs/SPEC.md#15-phase-4-activity-log)).

**Responsible use:** the app simulates input only on your own machine. You are responsible for following your organization's IT and acceptable-use policies.

## Command line

The app's own binary takes commands. They drive the app in the tray, and start it there if it isn't running.

```bash
unidle on              # keep awake, power assertion only (no input nudges)
unidle on --nudge      # also send the input nudges from Settings
unidle timer 30m       # or 1h30m, or 18:00
unidle status          # add --json for scripts
unidle off
```

**Links and shortcuts.** `unidle://on`, `unidle://off`, `unidle://timer/30m` and `unidle://timer/18:00` do the same, always power-only. On a Mac, add an "Open URLs" action with one of them to a Shortcut. On Windows, `integrations/powershell/Unidle` is a PowerShell module: copy the `Unidle` folder into a folder on `$env:PSModulePath`, then `Enable-Unidle`, `Start-UnidleTimer -Until 18:00`, `Get-UnidleStatus` or `Disable-Unidle`.

The `.deb` puts `unidle` on your `PATH`, and the AppImage takes the same commands. On Windows the binary is `unidle.exe` in the install folder, and on macOS it is `Unidle.app/Contents/MacOS/unidle`: add it to your `PATH` or link it. Details are in [SPEC §13](docs/SPEC.md#13-phase-4-command-line).

## On a work computer

Unidle doesn't hide. On a computer your employer or school manages, IT can usually see every installed app and running process, security tools may flag simulated input, and activity trackers are built to spot input that doesn't match real work. Unidle doesn't try to avoid any of this, and never will.

Keeping the screen on for a presentation or a long build is a common, harmless need, and power-only mode (Settings → Simulate input off, or `unidle on`) covers most of it with no simulated input at all. Using Unidle to appear active when you aren't working is a different matter: many employers treat it as misconduct, and people have lost their jobs over it. **Check your organization's IT and acceptable-use policies before installing it, or ask your IT team.** If Unidle finds signs that an organization manages your computer, it says so once and suggests power-only mode ([SPEC §16](docs/SPEC.md#16-phase-4-work-computers)).

**For IT:** Unidle makes no network connections and has no telemetry. To turn input simulation off and leave only the power assertion, set `DisableInputSimulation`:

- Windows: `HKLM\SOFTWARE\Policies\Unidle`, `DWORD` `DisableInputSimulation` = `1` (Group Policy or Intune).
- macOS: a configuration profile for the `com.throughthewind.unidle` domain with `DisableInputSimulation` = `true`.
- Linux: `/etc/unidle/policy.json` containing `{"disableInputSimulation": true}`.

## Develop

You need [Rust](https://rustup.rs) (stable), Node 22.18 or later, and the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
npm install
npm run dev
```

```bash
cargo test --workspace
npm run typecheck
node .ai/rungs.mjs check
npm run build
```

## Repository layout

| Path | What it holds |
| --- | --- |
| `crates/core/` | Simulation controller and platform adapter interface, with no UI dependency |
| `src-tauri/` | Tauri app shell: window, commands and packaging |
| `ui/` | Window UI in plain TypeScript, styled with the design tokens |
| `integrations/` | Automation for other tools, such as the PowerShell module |
| `site/` | One-page landing site, deployed to GitHub Pages by `.github/workflows/pages.yml` |
| `docs/` | [Spec](docs/SPEC.md), [roadmap](docs/ROADMAP.md), [design system](docs/design/DESIGN_SYSTEM.md), [decisions](docs/decisions/README.md), [studies](docs/studies/) and [backlog](docs/backlog/BACKLOG.md) |
| `.ai/`, `.claude/`, `AGENTS.md` | Development system installed by [rungs](https://github.com/ThroughTheWind/rungs). Not shipped in the app |

See [CONTRIBUTING](CONTRIBUTING.md) to get involved.

## License

[MIT](LICENSE)

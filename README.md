# Unidle

A small, open-source desktop app for Windows, macOS and Linux that keeps your computer awake with one ON/OFF toggle.

> **Status: in development.** Turning ON keeps the computer awake with an OS power assertion. Input nudges, the tray icon and settings are still to come. See the [roadmap](docs/ROADMAP.md).

## What it will do

When ON, the app asks the OS not to sleep and sends a tiny input event at an interval, with no visible effect, so apps that watch for input see activity too. When OFF, or if the app crashes, your computer behaves normally. It makes no network connections. The full behavior is in the [spec](docs/SPEC.md).

**Responsible use:** the app simulates input only on your own machine. You are responsible for following your organization's IT and acceptable-use policies.

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
| `docs/` | [Spec](docs/SPEC.md), [roadmap](docs/ROADMAP.md), [design system](docs/design/DESIGN_SYSTEM.md), [decisions](docs/decisions/README.md) and [backlog](docs/backlog/BACKLOG.md) |
| `.ai/`, `.claude/`, `AGENTS.md` | Development system installed by [rungs](https://github.com/ThroughTheWind/rungs). Not shipped in the app |

See [CONTRIBUTING](CONTRIBUTING.md) to get involved.

## License

[MIT](LICENSE)

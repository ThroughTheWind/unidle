---
id: WI-019
title: Drive Unidle from links, Apple Shortcuts and PowerShell
type: feature
status: review
branch: feature/WI-019-automation-hooks
created: 2026-11-05
updated: 2026-11-05
related: [WI-014]
epic:
children: []
---

## Proposal (rationale)

Phase 4's Could row for local automation hooks: a URL scheme (`unidle://on`), Apple Shortcuts, and a Windows PowerShell module. Each one opens the command line from WI-014 to another tool, without adding a new way for the app to run.

## Decision

`accepted` — 2026-11-05, Antoine ("Build the extras now" on the Phase 4 extras question).

## Plan

### Requirements

- SPEC §13, CL-6 and CL-7.
- Links are power-only, answer nothing, and reach the tray app like a command (CL-2).

### Impacts

- `src-tauri`: `cli::link_request`, a `Link` command in `cli::run`, `links.rs` for macOS link events and AppImage registration, `tauri-plugin-deep-link` with the scheme in `tauri.conf.json`.
- `integrations/powershell`: the module, its manifest and a test script; a CI step runs it on all three OSes.
- README and SPEC.

### Approach

- **Links go through the command line.** Windows and Linux start the binary with the link, so `cli::run` turns it into the same request a command sends, starting the tray app if needed. macOS hands the link to the app as an event, and the same parser handles it. Both platforms share one code path.
- **Links never nudge.** A web page can offer a link, so no link turns on input simulation or reads the status. The browser or OS asks before it opens one.
- **Shortcuts use links**, through "Open URLs". App Intents would need Swift and a native extension, which is out of proportion for this row.
- **PowerShell wraps the binary** and returns `--json` as objects, so it gets every future command for free.

### Acceptance criteria / tests

1. Unit tests: links parsed, power-only, unknown links refused.
2. Linux end to end: `unidle unidle://on` with the app not running starts it ON and power-only; a timer link; a bad link exits 2; `unidle://off`.
3. The PowerShell test script in CI on all three OSes: the manifest, the exports, and the arguments passed for each command.
4. On real machines: a link from a browser and from Shortcuts on macOS, and from Run (Win+R) on Windows after installing. **Needs real machines.**

### Out of scope

- App Intents for Shortcuts, and publishing the module to the PowerShell Gallery.

## Execution

- Branch `feature/WI-019-automation-hooks`, stacked on `feature/WI-018-network-conditions`.

## Review

1. Met.
2. Met.
3. Runs in CI.
4. **Open.** Needs real machines.

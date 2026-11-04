---
id: WI-012
title: Import and export the schedule as a file
type: feature
status: review
branch: feature/WI-012-schedule-import-export
created: 2026-11-04
updated: 2026-11-04
related: [WI-009, WI-011]
epic:
children: []
---

## Proposal (rationale)

ROADMAP Phase 2 lists importing and exporting schedules as a file (Could): a backup, or the same schedule on several machines.

## Decision

`accepted` — 2026-11-04, Antoine ("build phase 2"). Kept as its own PR so it can be dropped without touching the rest.

## Plan

### Requirements

- Settings has Import… and Export… for the schedule's times and days off. The on/off switch is not part of the file: importing never turns the schedule on by itself.
- The file is JSON with a version marker (`"unidleSchedule": 1`). An import is all or nothing: a file with any bad time or date is refused with a reason, and the current schedule stays.
- The app makes no network request (NFR-2), and the window gets no file or dialog permissions.

### Impacts

- `crates/core`: `Settings::schedule_file` and `Settings::schedule_patch_from_file`.
- `src-tauri`: `export_schedule` and `import_schedule` commands; new dependency `tauri-plugin-dialog` (which pulls `tauri-plugin-fs`), used from Rust only.
- `ui`: a Schedule file row.

### Approach

- The dialogs are opened from Rust, so the webview's capabilities stay as they were. The commands are async because the blocking dialogs must not run on the main thread.
- An import goes through `update_settings`, so it is validated and saved like any other change.

### Acceptance criteria / tests

1. Unit tests: the file round-trips without the switch; empty, non-object, unknown-version, bad-time and bad-date files are refused; missing lists mean empty.
2. On each OS with a real build: export, then import on a fresh settings file, gives the same times and days off. **Needs real machines.**

### Out of scope

- Merging an imported schedule into the current one: an import replaces it.

## Execution

- Branch `feature/WI-012-schedule-import-export`, stacked on WI-011.

## Review

1. Met. 2 new settings tests.
2. **Open.** Needs real machines.

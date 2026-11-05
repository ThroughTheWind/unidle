**Authoritative for:** which items exist and what state each is in, and the next free id.
**Not authoritative for:** why any item exists or how to do it — that is the item file.

# Backlog

The board. One row per live work item, grouped by status. Items live in
[`items/`](items/); finished work moves to [`archive/`](archive/).

<!-- NEXT-ID: WI-019 -->
<!-- Claim from this marker and bump it on your own branch. `node .ai/rungs.mjs check` refuses a duplicate. -->

## In progress

| Id | Title | Type | Branch |
| --- | --- | --- | --- |
| — | *nothing in flight* | | |

## Review

| Id | Title | Type | Branch |
| --- | --- | --- | --- |
| [WI-001](items/WI-001-power-assertion-spike.md) | Prove the power assertion on all three platforms | spike | `feature/WI-001-power-assertion` |
| [WI-002](items/WI-002-input-nudge-spike.md) | Prove pointer and keystroke nudges on all three platforms | spike | `feature/WI-002-input-nudge` |
| [WI-004](items/WI-004-tray-and-window-lifecycle.md) | Add the tray icon and hide-to-tray window lifecycle | feature | `feature/WI-004-tray-and-window` |
| [WI-005](items/WI-005-settings.md) | Store settings and apply them, with launch at login and a global shortcut | feature | `feature/WI-005-settings` |
| [WI-006](items/WI-006-landing-page.md) | Publish a one-page landing site with per-OS downloads | feature | `feature/WI-006-landing-page` |
| [WI-007](items/WI-007-window-ui.md) | Build the window and settings screen with the four power button styles | feature | `feature/WI-007-window-ui` |
| [WI-008](items/WI-008-app-icon.md) | Redesign the app and tray icons, with a Wake eye default and an App icon setting | feature | `feature/WI-008-app-icon` |
| [WI-009](items/WI-009-scheduling-core.md) | Decide ON/OFF between manual, timers and a weekly schedule in the core | feature | `feature/WI-009-scheduling-core` |
| [WI-010](items/WI-010-timers-and-source.md) | Add quick timers and the source line to the tray and window | feature | `feature/WI-010-timers-and-source` |
| [WI-011](items/WI-011-schedule-editor.md) | Edit the weekly schedule and days off in Settings | feature | `feature/WI-011-schedule-editor` |
| [WI-012](items/WI-012-schedule-import-export.md) | Import and export the schedule as a file | feature | `feature/WI-012-schedule-import-export` |
| [WI-013](items/WI-013-window-polish.md) | Let the timer row and tray click be turned off, drop the main title, polish form controls | feature | `feature/WI-013-window-polish` |
| [WI-014](items/WI-014-cli.md) | Add the unidle command line, power-only by default, driving the tray app | feature | `feature/WI-014-cli` |
| [WI-015](items/WI-015-conditions.md) | Hold activity on conditions: screen lock, sleep, AC power, battery, a running app | feature | `feature/WI-015-conditions` |
| [WI-016](items/WI-016-activity-log.md) | Keep an opt-in local activity log of ON sessions, with CSV export | feature | `feature/WI-016-activity-log` |
| [WI-017](items/WI-017-work-computers.md) | Warn once on a managed computer, and obey an IT policy that turns input simulation off | feature | `feature/WI-017-managed-device` |
| [WI-018](items/WI-018-network-conditions.md) | Hold activity unless on a chosen Wi-Fi network or a VPN | feature | `feature/WI-018-network-conditions` |

## Planned

| Id | Title | Type |
| --- | --- | --- |
| — | | |

## Accepted

| Id | Title | Type |
| --- | --- | --- |
| — | | |

## Proposed

| Id | Title | Type |
| --- | --- | --- |
| [WI-003](items/WI-003-measure-footprint.md) | Measure memory and installer size against the spec limits | spike |

## Deferred

| Id | Title | Revisit when |
| --- | --- | --- |
| — | | |

---

Rejected and completed items keep their files — see [`archive/`](archive/). A rejection is part of
the record, with its reason; deleting one loses the answer to "why not?" and invites the same
proposal again.

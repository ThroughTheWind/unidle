**Authoritative for:** which items exist and what state each is in, and the next free id.
**Not authoritative for:** why any item exists or how to do it — that is the item file.

# Backlog

The board. One row per live work item, grouped by status. Items live in
[`items/`](items/); finished work moves to [`archive/`](archive/).

<!-- NEXT-ID: WI-005 -->
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

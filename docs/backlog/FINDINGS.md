**Authoritative for:** what has been noticed and not yet decided, and the disposition of everything closed.
**Not authoritative for:** what to do about any of it — a finding becomes a work item before it becomes work.

# Findings

Things noticed while doing something else. **A finding is the observation; a work item is the
decision.** Recording one must cost almost nothing, or it will not happen — so a finding is a
**row**, not a file. Items are files; findings are rows. The asymmetry is deliberate.

<!-- NEXT-ID: F-003 -->

## Open

| Id | Sev | Pri | What | Evidence | When to act | How to fix |
| --- | --- | --- | --- | --- | --- | --- |
| F-002 | medium | next | The app exceeds the 80 MB memory limit (SPEC NFR-1) while its window is open, and meets it once the webview is closed | Windows 11 release build, whole process tree, 2026-11-03: window open 155 MB private and 338 MB working set; webview destroyed 6 MB private and 27 MB working set | Before WI-004 builds the final window | Apply the limit to the tray-only state in NFR-1 if Antoine agrees, and always destroy the webview when the window closes |
| F-001 | low | next | The spec and the design system disagree on the main window size | `docs/SPEC.md` FR-11 says about 320×240, `docs/design/DESIGN_SYSTEM.md` says 360×420. The skeleton uses 360×420 because the design's contents don't fit in 240 px high | Before WI-004 builds the final window | Change FR-11 to 360×420 if Antoine agrees |

## Closed

| Id | What | Disposition | Reason |
| --- | --- | --- | --- |
| — | | | |

---

## Recording one

Use `/record-finding`, or add a row directly. Required:

- **Sev** — `high` (wrong output, data loss, security) · `medium` (wrong behaviour, contained) ·
  `low` (cost, clarity, tidiness)
- **Pri** — `now` · `next` · `someday`. Severity is about the problem; priority is about us.
- **Evidence** — a path, a command, a count. **A finding with no evidence is a hunch**, and the
  next reader cannot tell the difference. If you cannot produce evidence, say so in the row.
- **When to act** — the trigger, not a date. *"Before the next release"*, *"if this recurs"*.
- **How to fix** — enough that someone else could, or an explicit "unknown".

## Closing one

Every finding leaves the Open table by one of three dispositions, and **each carries a written
reason**:

| Disposition | Means | Reason must say |
| --- | --- | --- |
| **promoted** | It became a work item | Which item, and what scope it took |
| **fixed** | It was resolved directly | What changed, and where |
| **dismissed** | It is not a problem, or not one worth solving | *Why not* — this is the one people skip, and it is the one that stops the same observation being recorded again next month |

`node .ai/rungs.mjs check` refuses a closed finding with no reason.

## What this register does not do

- It does not prioritise. A `now` priority is a claim by whoever typed it.
- It does not prove anything is fixed. `fixed` means someone said so.
- It counts what was **recorded**, not what was noticed. A quiet register and an unobserved repo
  look identical from here.

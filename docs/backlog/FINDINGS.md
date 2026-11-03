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
| — | | | *nothing open* | | | |

## Closed

| Id | What | Disposition | Reason |
| --- | --- | --- | --- |
| F-001 | The spec and the design system disagreed on the main window size | fixed | Antoine chose 360×420 on 2026-11-03, and SPEC FR-11 now says so |
| F-002 | The app exceeded the 80 MB memory limit while its window was open | promoted | Antoine agreed on 2026-11-03 that the limit applies only while hidden in the tray, and SPEC NFR-1 now says so. WI-004 takes the required webview release when the window closes |

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

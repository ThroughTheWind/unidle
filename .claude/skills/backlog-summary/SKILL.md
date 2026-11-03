---
name: backlog-summary
description: >-
  Summarize the backlog: open work items grouped by category, the highest-value items with the
  reasoning, stale or contradictory bookkeeping, and workflow-health observations with concrete
  suggestions. Use whenever asked to "summarize the backlog", "what's open", "backlog status /
  report / health", "rank the backlog", "what should we do next", or for any prioritization of
  open work — even when the word "backlog" is not used but the question is about what to build
  next. Read-only: it reports, it never changes an item. Executing one item is /work-item.
---

# Summarize the backlog

Read-only. **This skill never edits an item, a status, or the board.**

## 1. Read from the current tip, not a copy

Run against `main` as it is now. A worktree copy or a cached report goes stale,
and a summary built on one is confidently wrong about what is open.

## 2. Gather

Read every file in `docs/backlog/items/`, plus `BACKLOG.md`. Do **not** trust the board's grouping
— derive status from the item files and note any disagreement, which is itself a finding worth
reporting.

## 3. Report

### Open work, by category

Group by the natural categories of this repo (its subsystems, surfaces, or domains — not by
status). For each: count, and the items with a one-line summary each.

### Highest-value items, with the reasoning

Rank the top few, and **state why each ranks where it does** — value, cost, what it unblocks, what
it stops costing. A ranking without reasoning is an opinion wearing a number.

### Bookkeeping that disagrees with reality

Report, do not fix:

- items whose `status` and merged branch disagree
- items `in_progress` with no branch, or with a branch that no longer exists
- documents claiming they wait on work that has finished
- epics whose `children` links are not reciprocated

### Workflow health

Observations about the *process*, with a concrete suggestion each:

- items sitting in one status for a long time
- items whose acceptance criteria are blank while their status is past `planned`
- how much work arrives as items versus as unrecorded changes
- whether `items/` has outgrown readability and wants archiving

## 4. What this cannot tell you

State these limits in the output rather than leaving the reader to infer them:

- it measures **what is recorded**, not what is happening — work done without an item is invisible
- a ranking is a judgement, and the reasoning is there so it can be argued with
- an empty result and an unread backlog produce the same summary, so say how many files were read

A report that cannot fail loudly is not a measurement.

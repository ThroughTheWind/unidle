---
name: work-item
description: >-
  Execute a backlog work item (WI-###) end to end through its lifecycle: claim or open
  the item, branch, plan, implement, test, review, merge, and move the status. Use when asked to
  "work on / tackle / start / do / ship WI-###", to pick up the next item from the
  backlog, or to work an epic's sub-items. Also use to open a new item from a request that turns
  out to be real work rather than a quick fix. An out-of-scope observation is a finding, not an
  item — that is /record-finding. Shipping a version is /cut-release. This skill is one item,
  start to finish.
disable-model-invocation: true
---

# Execute a work item

**Authority:** [`docs/backlog/README.md`](../../../docs/backlog/README.md). Read it once per
session before the first item; this skill is the execution form, not the reasoning.

## 0. Establish the item

- Given an id → open `docs/backlog/items/WI-###-*.md`.
- Given a request with no id → decide first **whether this is an item at all**: a one-line fix with
  no design choice is just a change; an observation you are not acting on now is a **finding**. If
  it is an item, claim the next id from the `NEXT-ID` marker in `BACKLOG.md` and **bump the marker
  on your own branch**, then copy `TEMPLATE.md`.
- Given "the next item" → read the board, but **re-derive before committing to it**. A board row is
  bookkeeping about the work, not the work; scoping from a stale row is how work that is already
  done gets started again.

## 1. Check the status is real

Before doing anything, confirm the item's `status` and `branch` agree with git. If the branch is
merged and the status is pre-review, the work landed and the field lied — fix the field, say so in
`## Execution`, and stop. That is the whole task.

## 2. Plan, if it is not planned

An `accepted` item needs its plan filled before code: **requirements · impacts · approach ·
acceptance criteria · out of scope**. None may be blank.

- Acceptance criteria must be **checkable**. "Works correctly" is not a criterion; "given X, the
  endpoint returns 422 with a typed error" is.
- `Out of scope` is required — answer it or write `nothing deferred`. It is the section that stops
  an item quietly growing a second purpose.
- A significant design choice gets an ADR. Link it; do not restate it here.

Status → `planned`.

## 3. Branch and execute

```bash
git switch -c feature/WI-###-slug main
```

Status → `in_progress`.

- **Follow the repo's engineering guides**, not this skill, for how the code should look.
- **Work discovered mid-flight does not join this item.** A new item, or a finding. Record it and
  carry on — do not widen the branch.
- Deviations from the plan go in `## Execution` **with the reason**, as you make them. A plan that
  silently stopped matching what was built reads as verified and is not.

## 4. Test

Run the narrowest validation that covers what you touched, then the repo's standard gate set:

```bash
node .ai/rungs.mjs check
```

Existing tests are not weakened to make a change pass. If a test is genuinely wrong, that is its
own item with its own reasoning.

## 5. Review

Walk **each** acceptance criterion and write what you checked in `## Review`. A criterion you
cannot demonstrate is not met — leave it open and say so rather than marking the item done.

Status → `review`.

## 6. Land

Merge into `main`, **move the status to `done` in the same change**, then delete
the branch. In that order: the merged-status gate can only see items whose branch still exists, so
deleting first costs you the check.

Update the board: remove the row from **In progress**, and archive the item if the repo archives on
completion.

## When to stop and ask

- The plan cannot be written because the requirement is ambiguous in a way that changes the work.
- Execution reveals the approach is wrong — reopen the plan rather than improvising past it.
- Acceptance criteria conflict with something already shipped.

In each case, write what you found in the item first. The next session reads the item, not this
conversation.

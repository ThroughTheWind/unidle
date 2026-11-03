---
name: record-finding
description: >-
  Record something noticed while doing other work into the findings register — classified by
  severity and priority, with evidence, when to act, and how to fix — so it is not lost and does
  not derail the current task. Use when asked to "record / log / note a finding", "capture this for
  later", "don't lose this", "add that to the findings", or whenever work surfaces a real problem
  that is out of scope right now. Also use to triage the register: promote a finding to a work
  item, mark it fixed, or dismiss it with a reason. Executing tracked work is /work-item; changing
  an instruction so a mistake cannot recur is /harden-rule.
---

# Record a finding

**A finding is the observation; a work item is the decision.** This must cost almost nothing, or
you will keep working instead of recording — which is how the observation is lost.

## Recording

Add a row to the **Open** table in the register. Then **return to what you were doing.** Do not fix
it, do not scope it, do not open an item. Those are separate decisions made later, deliberately.

Fill every column:

- **Sev** — `high` (wrong output, data loss, security) · `medium` (wrong behaviour, contained) ·
  `low` (cost, clarity, tidiness). Severity is about the problem.
- **Pri** — `now` · `next` · `someday`. Priority is about us. They are independent: a high-severity
  problem in code nobody runs can be `someday`.
- **What** — one sentence, specific enough to act on without this conversation.
- **Evidence** — a path, a command, a count. **If you have none, write `none — hunch`.** A finding
  that looks evidenced and is not is worse than one that admits it, because the next reader spends
  an hour looking for the evidence.
- **When to act** — a trigger, not a date. *"Before the next release"*, *"if this recurs"*.
- **How to fix** — enough that someone else could, or `unknown`.

## What is not a finding

- **Something you are about to fix** — just fix it.
- **A rule that was broken** — that is `/harden-rule`. Recording it and leaving the instruction
  unchanged is how the same lesson gets re-learned by the next session.
- **Scope you decided against** — that belongs in the item's `Out of scope`.
- **A vague unease.** If you cannot say what is wrong specifically enough for someone else to
  check it, you have a feeling. Say so in the row or leave it out.

## Triaging

Every finding leaves **Open** by exactly one disposition, each with a written reason:

| Disposition | Do |
| --- | --- |
| **promoted** | Open a work item, link both ways, move the row to Closed naming the item and the scope it took |
| **fixed** | Resolve it, move the row naming what changed and where |
| **dismissed** | Move the row saying **why not** |

**Dismissal is the one that matters.** It is the disposition people skip, and its reason is what
stops the same observation being recorded again next month by someone who cannot tell it was
already considered. `node .ai/rungs.mjs check` refuses a closed finding with no reason.

## Before triaging, re-check

A finding has a shelf life. Re-derive it against the repo as it is now — a register row is a claim
about the past, and work has landed since. Findings that dissolve on contact get dismissed with
"already resolved by …", which is a perfectly good reason and a useful record.

---
id: WI-000
title: Short imperative title
type: feature          # feature | chore | docs | spike | epic
status: proposed       # proposed | accepted | rejected | deferred | planned | in_progress | review | done
branch:                # feature/WI-000-slug — set when execution starts; epics have none
created: YYYY-MM-DD
updated: YYYY-MM-DD
related: []            # WI-###, ADR-####, spec ids
epic:                  # parent WI-### — sub-items only
children: []           # child WI-### — epics only
---

## Proposal (rationale)

The problem, and why it matters. Expected value, or the cost of not doing it.

## Decision

`accepted` | `rejected` | `deferred` — date, who, and the reason. Empty until decided.

## Plan

> Filled once `accepted`. Significant design choices get an ADR — link it.

### Requirements

- Explicit and testable.

### Impacts

- Specs and ADRs touched, code areas, data and contracts, migration or ops, risks.

### Approach

- The chosen approach; options considered and their trade-offs; decisions still open.

### Acceptance criteria / tests

- How we will know it is done and correct.

### Out of scope

- **Required — answer, do not delete.** What this item explicitly does *not* cover, plus the
  follow-up item id if there is one, or an explicit `nothing deferred`.
>
> A blank line here is an unfinished plan, exactly like blank acceptance criteria. This section is
> the one that prevents an item quietly growing a second purpose.

## Execution

Branch, key commits, notes, and any deviation from the plan **with its reason**.

## Review

Verification against each acceptance criterion. What was checked, and how.

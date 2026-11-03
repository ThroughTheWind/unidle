---
id: ADR-0000
title: Short imperative statement of the decision
status: proposed        # proposed | accepted | rejected | superseded
date: YYYY-MM-DD
supersedes:             # ADR-#### — when this replaces an earlier record
superseded_by:          # set when a later record replaces this one
---

# ADR-0000 — Title

## Context

The forces at work: what is true, what is constrained, what problem is being solved. Enough that
someone who was not here can tell why a decision was needed at all.

> If this is one sentence, check admission criterion 1 — you may be describing work rather than
> constraining it.

## Decision

Stated in the present tense, as a rule: *"X is Y"*, not *"we will make X be Y"*.

## Consequences

**Good**

- What this makes possible or cheap.

**Costs and risks**

- What it makes harder, and what could go wrong.

> **Both halves are required.** A consequences section with no costs is advocacy, and the next
> reader will assume the costs were never considered.

## Alternatives considered

**\<Alternative\>** — what it was, and **the reason it lost**.

> This is the section future readers actually come for. "We considered other options" without
> naming them is the same as not having considered them.

## Revisit triggers

Facts about the world that would reopen this — not a date. A record with no trigger is one nobody
will know to revisit.

1. …

## Admission check

Against [the rule](README.md): (1) constrains future work ✅ · (2) a real alternative was rejected
with a stated reason ✅ · (3) reversing later costs meaningfully more ✅ · (4) not owned by another
document ✅ · (5) not an implementation detail ✅.

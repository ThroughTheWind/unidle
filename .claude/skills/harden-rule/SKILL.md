---
name: harden-rule
description: >-
  Turn a mistake into a rule that cannot be made again, by picking the cheapest rung that actually
  holds: a sentence at the point of use, a line in a skill, a path-scoped rule, or a gate or hook.
  Use when you got something wrong that an instruction could have prevented, when the same
  mistake happens twice, when a rule that already exists was broken anyway, or when asked to
  "write that down", "make sure this doesn't happen again", or "add a check for this". Also use to
  decide whether a rule is worth a gate at all. Recording the observation without changing the
  instruction is /record-finding; this skill changes the instruction.
---

# Harden a rule

**A mistake an instruction could have prevented is a defect in the instruction.** Repairing it
belongs in the same change as repairing the mistake — nobody will ask, because a reviewer reads the
fix, not the counterfactual in which you had been told the right thing first.

Run this unprompted. It is not a request; it is part of finishing.

## 1. Name the shape, not the incident

Write the rule as something the *next* agent can recognise before making the mistake, not as a
description of what you did. The trigger has to be mechanical: you should not need to notice you
were careless, only to notice a shape.

Weak: *"be careful when editing the dataset."*
Strong: *"reconcile a generated artifact by regenerating it, never by merging text."*

If you cannot state the shape, you have a finding, not a rule. Stop here and record it.

## 2. Check whether the rule already exists

Search the entry document, the rules, and the skills for it.

**If it already existed and was broken anyway, skip to rung 3 or 4.** Do not restate it, do not
bold it, do not add an emphasis marker. A louder sentence in a file that was already read changes
nothing, and the restatement is itself evidence that prose has been tried.

## 3. Pick the cheapest rung that holds

| Rung | Use when | Cost |
| --- | --- | --- |
| **1 — a sentence at the point of use** | First occurrence, and the rule is local to one place | Minutes |
| **2 — a line in the relevant skill** | The mistake happens *during* a procedure that already has a skill | Minutes |
| **3 — a path-scoped rule** in `.ai/rules/` | It applies to a surface rather than a task, and would be noise in the always-on document | An hour |
| **4 — a gate or a hook** | **The rule has already been broken after being written down**, or the check is mechanical and cheap | Half a day, plus maintenance |

Rung 1 goes **where the mistake is made** — not in a preamble, not in a summary section. The rule
has to be in front of the person about to break it.

## 4. If it is rung 4, build it properly

Add the gate to the owning module's table, or as a `command` gate in `.ai/gates.toml`. Then, before
you call it done:

- **Self-tests, both directions.** One expecting `fail`, one expecting `pass`. A gate whose rules
  are all currently satisfied is indistinguishable from a gate that matches nothing.
- **Read the negation before the token.** If the gate refuses a phrase, check a preceding-context
  window for a negation cue first — otherwise it refuses the sentence that documents the fix, and a
  guard that refuses its own fix is one people disable.
- **An exemption must carry a reason.** `<marker>: <why>`, ignored when the reason is missing.
- **Recompute rather than compare.** If the gate checks a number, derive the number. A probe
  encoding a guess is confidently wrong, which is worse than a typed value nobody trusts.
- **Pin what it does not cover**, in its own message. Green must never read as "verified".

## 5. Write down where it is now true

A rule usually lives in more than one place: the authority document that explains *why*, the
always-on document that every session reads, and the skills that execute it.

**Fix the authority first, then the citers.** A citer corrected against a stale authority is a
second wrong statement, and the next reader cannot tell which one won.

**A citation is not propagation.** "See §3" ages into a false claim the moment §3 changes, and it
reads as verified precisely because it names a source.

## 6. Say what you did

In the same change: what went wrong, which rung you chose, and why the cheaper rungs were not
enough. That last part is what stops the next person re-litigating it — and if the honest answer is
"prose had already failed twice", write that.

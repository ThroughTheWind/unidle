# Path-scoped rules

**These files are the source. The copies under `.claude/rules/`, `.github/instructions/` and
`.cursor/rules/` are generated — do not edit those.** Run `node .ai/rungs.mjs render` after changing
anything here; `node .ai/rungs.mjs check` refuses a stale rendering.

A rule here loads only when an agent touches a matching file, so it costs nothing until it is
relevant. That is what keeps [`../../AGENTS.md`](../../AGENTS.md) inside its line budget.

## When something belongs here

| Content | Goes |
| --- | --- |
| A fact every session needs | `AGENTS.md` |
| A rule for one part of the tree | **here** |
| A multi-step procedure with a beginning and an end | a skill |

If you are adding a section to `AGENTS.md` that begins "when working on X…", it belongs here.

## Format

```markdown
---
description: >-
  What this covers and when it applies. One or two sentences — some harnesses use this to decide
  whether to load the rule at all, so lead with the trigger.
paths:
  - "src/api/**/*.ts"
  - "tests/api/**"
enforcement: gated        # gated | review-only
---

# API rules

- Concrete, checkable statements. "Use 2-space indentation", not "format properly".
```

- **`paths`** — globs. Omit for a rule that should load every session, but prefer `AGENTS.md` for
  that; a rule with no paths is a rule with no reason to be here.
- **`enforcement`** — required. `gated` means a gate enforces it; `review-only` means nothing does.
  There is no silent third category, because a silent third category is what every repo this
  content came from actually had.

## What does not survive rendering

Not every harness can express every field, and the render report names each loss:

| Field | Claude | Copilot | Cursor | AGENTS.md-only |
| --- | --- | --- | --- | --- |
| `paths` | ✅ | ✅ `applyTo` | ✅ `globs` | directory-level, or a routing line |
| `description` | dropped | ✅ | ✅ | — |

Read `.ai/render-report.md` after a render to see what your harness set actually received.

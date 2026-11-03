---
name: close-session
description: >-
  Close out a working session: write the dated archive note, reset the live handoff document to a
  cold-readable resume point, and record what became an active constraint. Use when asked to
  "close the session", "wrap up", "archive this session", "hand off", "write the handoff", or at
  the end of a long stretch of work before context is lost. Recording an out-of-scope observation
  is /record-finding; finishing a tracked item is /work-item.
disable-model-invocation: true
---

# Close a session

The test for everything below: **could a session starting cold tomorrow, with no memory of this
conversation, pick up from the document alone?** If not, it is not closed.

The handoff is narrative continuity, not a checkpoint, conversation memory, event stream, or audit
record. An archive can point to evidence, but events and persisted history do not establish actor
identity, retention, access control, request binding, or decision reasons by themselves.

## 1. Write the archive note

`.ai/archive/YYYY-MM-DD_session-NN_<what-closed>-and-<what-is-next>.md`. The filename carries both
halves so the directory is scannable without opening anything.

Contents:

- **Delivered** — what actually landed, with ids.
- **Not delivered** — what was planned and did not happen, **and why**. This is the half people
  skip, and it is the half that stops the next session re-planning the same thing.
- **Decisions taken**, with reasons. Flag any that should become an active constraint.
- **What turned out to be wrong** during the session, and what replaced it.

Write what was believed at close. **Never revise an archived note later** — its value is that it
records what was known then.

## 2. Rewrite the live document

`.ai/session.md` is **rewritten, not appended to.** Delete what is no longer true rather than layering
qualifications on it.

- **Resume from** — one concrete next action. A path, a command, an id. Not "continue the
  refactor"; something that can be executed.
- **Active constraints** — move any decision here that a fresh session would otherwise relitigate,
  each with why it is closed. Be specific: *"the storage boundary is fixed by ADR-0007, do not
  re-scope it"* beats *"architecture is settled"*. **This section is what pays for the document.**
- **Working assumptions** — what is believed but unverified. Being wrong here is expected; the
  value is telling the next session which claims to distrust first.
- **Open questions** — mark which block progress. "None blocking" is worth stating.

## 3. Check it against the archive

The live document should now say nothing the archive note repeats. If it does, one of them is
wrong later: **the live document is present tense, the archive is past tense**, and a fact in both
will drift in one.

## 4. What not to do

- **Do not close a session to avoid finishing something.** If work is half-done, say so in
  `In progress` with what remains — a handoff that hides a loose end is worse than no handoff.
- **Do not summarise the conversation.** The archive records outcomes and decisions, not a
  narrative. Nobody reads a transcript.
- **Do not promote an assumption to a constraint** because it went unchallenged. A constraint is
  something decided; an assumption is something believed.

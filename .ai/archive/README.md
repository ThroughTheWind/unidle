# Session archive

Closed sessions, one file each, newest first in the index generated into
[`.ai/session.md`](../session.md).

## Naming

`YYYY-MM-DD_session-NN_<what-closed>-and-<what-is-next>.md`

The filename does the work. Someone scanning this directory should be able to find the session
that closed a given piece of work without opening anything — which is why the name carries both
halves, not just a number. `node .ai/rungs.mjs check` refuses a name that does not.

## What an archive note holds

- What was delivered, and what was **not** delivered that was planned
- Decisions taken, with their reasons — especially ones that became active constraints
- The handoff state at close: what the next session was told to do
- Anything that turned out to be wrong during the session, and what replaced it

## What it is not

**An archive note is a record, not a source of truth.** It says what was believed at a moment. If
it disagrees with a spec, a decision record, or the code, those win — and the disagreement is
worth a finding, because it means something changed without its authority being updated.

**Never edit an archived note** to reflect what was learned later. Its value is that it is what
was known then.

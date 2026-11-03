**Authoritative for:** what lives in this directory and when an item leaves it.
**Not authoritative for:** the lifecycle itself, which is the methodology one level up.

# Items

One file per work item: `WI-###-slug.md`, from
[`../TEMPLATE.md`](../TEMPLATE.md).

This directory holds **only work that can still change.** Finished items move to
[`../archive/`](../archive/) with `node .ai/rungs.mjs backlog archive`, which recomputes every link repo-wide so
archived ids still resolve and stay permanently spent.

Never edit an archived item. If archived work turns out to be wrong, that is a new item.

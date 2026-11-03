**Authoritative for:** how finished items are archived and why they are never edited afterwards.
**Not authoritative for:** the content of any archived item, which is fixed at the moment it was archived.

# Archive

Finished work items, moved here so [`../items/`](../items/README.md) holds only work that can still
change.

Move them with `node .ai/rungs.mjs backlog archive`, never by hand — it recomputes every link repo-wide, and
archived ids stay indexed so citations to them keep resolving and the ids stay permanently spent.

**Never edit an archived item.** If archived work turns out to be wrong, that is a new item. The
value of an archived record is that it says what was decided at the time.

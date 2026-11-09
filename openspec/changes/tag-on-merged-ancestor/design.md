# Design

## Decisions

1. **Compare the version, never the tree.**
   The tagged commit counts when HEAD contains it and its manifest holds the version HEAD has.
   Requiring the tagged commit's tree to equal the version commit's tree was considered and lost.
   It rejects the ordinary case it was meant to admit: a later forkctl publish that changes a file other than the manifest, such as a workflow, gives the first-parent commit at the same version a different tree.
   The registry is the record of what was released, and it already holds the version.
2. **"Contains" is the merge-base test.**
   HEAD contains the tagged commit when some first-parent commit's merge-base with it is the tagged commit itself.
   Every first-parent commit newer than the merge that brought it in contains it too, so the walk stops at the first one that does not.
   A walk over every ancestor of HEAD was considered and lost: it reads the whole history to answer for one commit, where the merge-base test stops at the merge.
3. **The depth is the merge's.**
   Depth orders releases oldest first.
   The merge is where the tagged commit reached this branch, as a merge counts for `version_commit`.
   Using the version commit's depth was considered and lost: the tagged commit's history can predate it.

Override and disable path: none is needed.
The rule widens only which existing tag counts as the release; a tag on a commit at another version, or on a commit HEAD does not contain, still fails as before.

# Design

## Decisions

1. **Compare the version.**
   - The tagged commit counts when HEAD contains it and its manifest holds the version HEAD has.
   - Requiring the tagged commit's tree to equal the version commit's tree was considered and lost.
     It rejects the ordinary case it was meant to admit.
     A later forkctl publish can change a file other than the manifest, such as a workflow.
     That gives the first-parent commit at the same version a different tree.
   - The registry is the record of what was released, and it already holds the version.
2. **"Contains" is the merge-base test.**
   - HEAD contains the tagged commit when some first-parent commit's merge-base with it is the tagged commit itself.
   - Every first-parent commit newer than the merge that brought it in contains it too, so containment is monotone along the first-parent line.
   - The search tests HEAD, then the commits 1, 2, 4, 8, … steps below it, until one does not contain the tagged commit.
     It then bisects that last interval.
   - A merge `d` commits below HEAD then costs O(log d) merge-base walks.
     Each forkctl publish after a release adds one to `d` until the next version bump.
   - A linear walk from HEAD to the merge was considered and lost: it costs `d + 1` walks.
   - A binary search over the whole first-parent line was considered and lost.
     That line holds the entire history when the tag is off it.
     So the search costs about log2 of the history length even when the merge is HEAD's parent.
   - A walk over every ancestor of HEAD was considered and lost: it reads the whole history to answer for one commit.
3. **The depth is the merge's.**
   Depth orders releases oldest first.
   The merge is where the tagged commit reached this branch, as a merge counts for `version_commit`.
   Using the version commit's depth was considered and lost: the tagged commit's history can predate it.

Override and disable path: none is needed.
The rule widens only which existing tag counts as the release.
A tag on a commit at another version, or on a commit HEAD does not contain, still fails as before.

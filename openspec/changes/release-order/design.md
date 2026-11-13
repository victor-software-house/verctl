# Design

The research note `docs/research/2026-11-13-atomic-releases.md` holds the evidence for each decision.
The operator made each decision on 2026-11-13.

## 1. Order: draft, registries, then the tag

**Decision.**
The run builds the assets and uploads them to a draft Release, publishes to the registries in dependency order, then publishes the draft, which creates the tag.
A draft can still be changed or deleted, so every reversible step comes first.

**Alternative.**
Tag before the uploads, as `release-tag-order` requires today.
It lost because a tag then exists for a release that may never complete.

**Override.**
None: the order holds for every package kind.
A package with no registry check still tags after its upload, as today.

## 2. A partial release is a state that a rerun resumes

**Decision.**
`status` reports a release with some but not all uploads as partial, with the count, the cause, and the resume time when the registry sent one.
A rerun skips every crate the registry index lists and resumes with the rest, then publishes the draft.

**Alternative.**
Treat a partial release as a failed run to redo by hand.
It lost because cargo does not retry an upload on a 429 and the crates already uploaded stay live, so only verctl can tell what is left.

**Disable.**
None: the state is read from the registry, so it needs no setting.

## 3. The asset legs build, and `release` uploads

**Decision.**
Each asset leg runs `release asset`, which builds its tarball and uploads nothing.
The workflow hands the tarballs to the publish job as run artifacts.
There `verctl release --assets-dir <dir>` creates the draft at HEAD, uploads every declared tarball from the directory, publishes to the registries, and then publishes the draft.
One process owns the whole order, and the `finalize` job goes.
A run whose declared tarballs are not all in the directory fails before the draft and before any upload.

The draft is created with no tag and with `target_commitish` set to HEAD.
GitHub creates the tag when the draft is published.
A draft found at another commit is deleted and created again at HEAD, since its tarballs were built from other code.
The operator chose this on 2026-11-13.

**Alternative.**
A `release draft` job creates the draft before the asset matrix, and each leg uploads its own tarball, as `release asset` does today.
It lost because the order would then span three jobs, and no single test could prove it.

## 4. The cause of a stop is kept in the draft

**Decision.**
When an upload fails, the run writes the stop into the draft Release's body before it exits: the crate, the cause, and the time the registry gave for the next attempt.
The record is an HTML comment that only verctl writes, so the rendered page does not show it.
`status` reads it from the draft it already looks up, and publishing the draft writes the notes again, which drops it.

Anyone who can edit the draft can edit the record, so it is display only.
`status` shows it as what the last run reported, and no decision reads it: what is live always comes from the registry.
A record that does not parse is reported as unreadable, never trusted in part.
The operator chose this on 2026-11-13.

**Alternative.**
Keep the cause only in the failed run's log and job summary.
It lost because `status` in a later run could then name the count but not the cause.

## 5. Rate limits

Trusted-publishing uploads count against no crates.io limit, so a release of existing crates meets none.
Only a first publish meets the new-crate limit of 5, then 1 every 10 minutes; [`first-publish`][first-publish] waits it out.

[first-publish]: ../first-publish/proposal.md

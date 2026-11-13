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

## 3. Rate limits

Trusted-publishing uploads count against no crates.io limit, so a release of existing crates meets none.
Only a first publish meets the new-crate limit of 5, then 1 every 10 minutes; [`first-publish`][first-publish] waits it out.

[first-publish]: ../first-publish/proposal.md

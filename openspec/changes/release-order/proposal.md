# Release order

## Motivation

A release tags its commit before the registry uploads today.
When an upload then fails, as `shrub-process` 0.7.0 did on crates.io, the tag names a release that never completed, and the uploads already made stay live.
A draft Release can still be changed or deleted, a registry upload cannot, and a published tag reads as a finished release.

The research note `docs/research/2026-11-13-atomic-releases.md` holds the analysis and the operator's decisions of 2026-11-13.
This change follows [`release-from-head`][release-from-head].

Queue row: [VER-134][ver-134].

## Changes

1. **Order.**
   A run builds the assets and uploads them to a draft Release, publishes to the registries in dependency order, then publishes the draft, which creates the tag at HEAD.
2. **A partial release** is a state `status` reports, with its count and its cause, and a rerun resumes it.
3. **Rate limits.**
   Trusted-publishing uploads count against no crates.io limit, so a release of existing crates meets none, and a rerun after another failure skips every crate the index lists.

[release-from-head]: ../release-from-head/proposal.md
[ver-134]: ../../../tasks.yaml

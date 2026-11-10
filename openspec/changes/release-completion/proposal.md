# State how each kind of release completes

## Why

A release that publishes nothing, a release with assets, and a package's
first publication each end differently. Today the `publish` job creates the
Release before the `assets` job uploads to it. GitHub immutable Releases
(GA 2025-10-28) refuse an asset once a Release is published, so a repository
with assets cannot turn them on.

Queue row: VER-011. This change is one part of
[release-state-machine][rsm]; the stages and transitions it completes are in
[derived-release-state][drs].

## What Changes

1. A release with assets is a draft until they are all uploaded.
2. A release that does not publish has no `publish` transition. Without
   assets it is complete at `merged`, with no tag and no Release.
3. Whether a release publishes is read from its release commit, and nowhere
   else.
4. A first publication that the registry needs is a stop, `bootstrap`, never
   a guess.

## Capabilities

### New Capabilities

- `release-completion`: drafts, the first publication, releases that do not
  publish, and where that setting is read.

### Modified Capabilities

None.

## Impact

1. Code: `src/release.rs` and `src/github.rs`.
2. Ledger: PR 228 built a draft-and-finalize slice on the 0.7 schema and is
   closed unmerged; the VER-011 notes list what to reuse from it.

[rsm]: ../release-state-machine/proposal.md
[drs]: ../derived-release-state/proposal.md

# First publish

## Motivation

crates.io trusted publishing cannot create a crate: its first version needs a user's token, and a trusted publisher entry can only be added to a crate that exists.

shrub 0.7.0 stopped there, on the new crate `shrub-process`, after another crate of the release was live.
A first publish is a manual act today, with no verctl command, and nothing then registers the trusted publisher.

The research note `docs/research/2026-11-13-atomic-releases.md` holds the analysis and the operator's decisions of 2026-11-13.
This change follows [`release-checks`][release-checks], whose `registry` check reports a missing crate.

Queue row: [VER-136][ver-136].

## Changes

1. **`verctl release --new`** lets a release create the crates crates.io lacks, with the registry token, at the version the default branch holds.
   It is the first of two atomic operations the operator chose on 2026-11-13.
   Every other part of the release stays as it is: its proofs, its draft Release, and the tag that the draft's publish creates.
2. **`verctl release trust [CRATE]...`** lists each crate's trusted publisher entries and adds this repository's entry only when it is missing.
   `release trust --dry-run` shows what it would add.
   The `trust` check of `release-checks` proves the entry, so `check` proves and a `release` step acts.
3. **`verctl release bootstrap [CRATE]...`** is the shorthand for the two in a row: `release --new` for the crates named, or every crate crates.io lacks, then `release trust` for each crate that run created.
   The operator chose that on 2026-11-14, replacing a separate step that carried its own upload and tag code.
4. **The first version is the one the default branch holds**: `0.0.0` for a standalone package, the group's version for a member of a shared-version group.
5. **Rate limits.**
   A release the new-crate limit stops records the time and a rerun resumes it, and `release trust` waits on a 429 until `Retry-After`.
6. **A token in CI is opt-in** through the `publish-new` input, which runs `release --new`; trusted publishing stays the default.
7. **No placeholder** is ever published to hold a name.

## Release

A breaking patch, through a `patch!` fragment listed under `### Breaking` (operator, 2026-11-13).
The steps and the `publish-new` input are additions, but `check` now fails a never-published standalone package at a version other than `0.0.0`, where it passed before.

[release-checks]: ../release-checks/proposal.md
[ver-136]: ../../../tasks.yaml

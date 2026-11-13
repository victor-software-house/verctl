# First publish

## Motivation

crates.io trusted publishing cannot create a crate: its first version needs a user's token, and a trusted publisher entry can only be added to a crate that exists.
shrub 0.7.0 stopped there, on the new crate `shrub-process`, after another crate of the release was live.
A first publish is a manual act today, with no verctl command, and nothing then registers the trusted publisher.

The research note `docs/research/2026-11-13-atomic-releases.md` holds the analysis and the operator's decisions of 2026-11-13.
This change follows [`release-checks`][release-checks], whose `registry` check reports a missing crate.

Queue row: [VER-136][ver-136].

## Changes

1. **`verctl release bootstrap [PACKAGE]...`** publishes, with the operator's token, the real code of each named package the registry lacks, or of every such declared package when none is named.
   It publishes at the version the default branch holds, tags each standalone package, and runs `release trust`.
   It is a step under `release`, as `release asset <target>` is, and the name [`derived-release-state`][derived] already plans.
2. **`verctl release trust [PACKAGE]...`** lists each crate's trusted publisher entries and adds this repository's entry only when it is missing.
   `release trust --dry-run` shows what it would add.
   The `trust` check of `release-checks` proves the entry, so `check` proves and a `release` step acts.
3. **The first version is the one the default branch holds**: `0.0.0` for a standalone package, the group's version for a member of a shared-version group.
4. **Rate limits.**
   On a 429 both steps wait until `Retry-After`, so one run creates any number of crates.
5. **A token in CI is opt-in** through the new `bootstrap` input; trusted publishing stays the default.
6. **No placeholder** is ever published to hold a name.

## Release

A breaking patch, through a `patch!` fragment listed under `### Breaking` (operator, 2026-11-13).
The two steps and the `bootstrap` input are additions, but `check` now fails a never-published standalone package at a version other than `0.0.0`, where it passed before.

[derived]: ../derived-release-state/proposal.md
[release-checks]: ../release-checks/proposal.md
[ver-136]: ../../../tasks.yaml

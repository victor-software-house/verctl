# First publish

## Motivation

crates.io trusted publishing cannot create a crate: its first version needs a user's token, and a trusted publisher entry can only be added to a crate that exists.
shrub 0.7.0 stopped there, on the new crate `shrub-process`, after another crate of the release was live.
A first publish is a manual act today, with no verctl command, and nothing then registers the trusted publisher.

The research note `docs/research/2026-11-13-atomic-releases.md` holds the analysis and the operator's decisions of 2026-11-13.
This change follows [`release-checks`][release-checks], whose `registry` check reports a missing crate.

Queue row: [VER-136][ver-136].

## Changes

1. **`verctl release --new`** publishes, with the operator's token, the real code of each package the registry lacks, at the version the default branch holds, tags each standalone one, and runs `verctl trust`.
2. **`verctl trust`** lists each crate's trusted publisher entries and adds this repository's entry only when it is missing.
   `trust --dry-run` shows what it would add.
3. **The first version is the one the default branch holds**: `0.0.0` for a standalone package, the group's version for a member of a shared-version group.
4. **Rate limits.**
   On a 429 both commands wait until `Retry-After`, so one run creates any number of crates.
5. **A token in CI is opt-in** through the new `publish-new` input; trusted publishing stays the default.
6. **No placeholder** is ever published to hold a name.

[release-checks]: ../release-checks/proposal.md
[ver-136]: ../../../tasks.yaml

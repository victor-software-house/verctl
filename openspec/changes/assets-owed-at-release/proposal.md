# A release owes the assets its own commit declared

## Motivation

Adding an asset target reopens every earlier release.
`pending::pending` builds the targets each release owes from `.ctl/ver.yaml` at HEAD ([`src/pending.rs`][pending]).
A release made before the target existed then lacks it, so the release counts as incomplete.

A caller hit this on 2026-11-11.
One commit moved it to verctl 0.8.0, added a `darwin-arm64` target, and added a patch fragment.
The plan then listed its `0.0.1` release, which holds only the `linux-x64` tarball, as incomplete.
The publish job checked out that release's commit, which pins verctl 0.7.4, and failed on `unrecognized subcommand 'release'`.
The asset jobs and the Version PR job were skipped, so no Version PR opens, and every push to the default branch fails the same way.

Queue row: [VER-122][ver-122].

## Changes

1. A release owes only the asset targets that `.ctl/ver.yaml` declared at its version commit, and that HEAD's config still declares.
2. A version commit with no `.ctl/ver.yaml` owes no assets.
3. The config at an old commit is read only when the release lacks a target HEAD declares, so a complete release never needs its old config to parse.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `idempotent-publish`: which assets a release owes.

## Impact

1. Code: `src/pending.rs`, `src/git.rs` (a file at a commit), tests.
2. Bundle: `verctl instructions` and the operator skill, through their templates.
3. Callers: none must change. A caller blocked by this bug releases again once it runs the fixed verctl.
4. Release: a patch.

[pending]: ../../../src/pending.rs
[ver-122]: ../../../tasks.yaml

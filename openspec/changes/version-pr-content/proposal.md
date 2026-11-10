# Require a Version PR to be the version step and nothing else

## Why

Identity alone trusts whoever pushes to the proposal branch, and a label or a
branch name shows nothing about what a Version PR changes. A hand edit can ride
a release.

Queue row: VER-011. This change is one part of
[release-state-machine][rsm]; who may open a Version PR is
[`version-pr-identity`][vpi]. It folds in an approved intent of the plan once
held on `feat/011-refresh-lockfile`: the version step refreshes lockfiles
itself.

## What Changes

1. The version step writes every file a release changes: manifests,
   dependent requirements, lockfiles, CHANGELOG sections, and served files.
2. `check` verifies that a Version PR's diff is the version step applied to
   its base and nothing else.
3. Lockfiles and served files on a Version PR are what the plan implies, and
   nothing more.

## Capabilities

### New Capabilities

- `version-content`: what the version step writes, and what a Version PR may
  change.

### Modified Capabilities

None.

## Impact

1. Code: `src/versions.rs`, `src/release.rs`, and the lockfile parsers.

[rsm]: ../release-state-machine/proposal.md
[vpi]: ../version-pr-identity/proposal.md

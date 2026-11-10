# Require a Version PR to be the version step and nothing else

## Motivation

Identity alone trusts whoever pushes to the proposal branch, and a label or a
branch name shows nothing about what a Version PR changes. A hand edit can ride
a release.

Queue row [VER-095][ver-095]. This change is one part of
[release-state-machine][rsm]; who may open a Version PR is
[`version-pr-identity`][vpi]. It folds in an approved intent of the plan once
held on `feat/011-refresh-lockfile`: the version step refreshes lockfiles
itself.

## Changes

1. The version step writes every file a release changes:
   - manifests;
   - lockfiles through their owners;
   - CHANGELOG sections;
   - pinned and served files;
   - the removal of every consumed fragment, one that bumps nothing
     included.
2. `check` verifies that a Version PR's diff is the version step applied to
   its base and nothing else.
3. Lockfiles and served files on a Version PR are what the plan implies, and
   nothing more.
4. `check` fails a pull request that is not a Version PR when it adds a
   CHANGELOG section. It also fails any pull request that changes whether a
   package publishes without a fragment for it. These are the remaining rules of
   [`version-pr-identity`][vpi] task 3.2. Both make `check` stricter, so they
   ship in 0.8.0 rather than force 0.9.0 later.

Rewriting a dependent requirement the new version no longer admits is queue
row [VER-105][ver-105]. It widens what the version step writes, so it can
ship after 0.8.0; until then `check` refuses a requirement change that no
pin explains.

## Capabilities

### New Capabilities

- `version-content`: what the version step writes, and what a Version PR may
  change.

### Modified Capabilities

None.

## Impact

1. Code: `src/prepare.rs`, `src/detect.rs`, `src/versions.rs`,
   `src/release.rs`, and the lockfile parsers.

[rsm]: ../release-state-machine/proposal.md
[ver-095]: ../../../tasks.yaml
[ver-105]: ../../../tasks.yaml
[vpi]: ../version-pr-identity/proposal.md

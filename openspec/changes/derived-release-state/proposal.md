# Derive release state from facts on every run

## Motivation

verctl has no model of a release, so each command rebuilds what it needs from
indirect signals:

1. A rerun of `publish` after the default branch moves can create the tag on a
   commit that was never published.
   The cause: "this commit is a release" is a CHANGELOG heading plus ancestry,
   which is true for every later commit.
2. "Already published" is decided by matching six substrings of registry
   stderr.

The prior art in [research.md][research] shows the same failure in the most
used tools. Each marks a release done from one signal, such as a label, a
tag, or a registry entry. Each then loses a step after a partial failure.

Queue row: [VER-011][ver-011].
This change is one part of [release-state-machine][rsm], which holds the shared
goals, decisions, and migration.

## Changes

1. A release is derived on every run from these facts:
   - fragments
   - manifests
   - the release commit in branch history
   - lockfile agreement
   - the registry
   - the tag
   - the GitHub Release, and its assets

   It keeps no state file, label, or event payload.
2. Every transition is guarded and repeatable: `propose`, `publish`, `tag`,
   `release`, `asset`, `finalize`. Publish precedes tag for a package with
   an unchecked upload, and a registry-checked package is tagged first
   ([tag-at-merge](../archive/2026-10-24-tag-at-merge/proposal.md)). Tags land on the
   release commit in one atomic push.
3. **BREAKING:** commands become `status`, `check`, `version`, `propose`,
   `release`, `release asset`, `release bootstrap`, `render`, `ci`, and
   `instructions`. Not started; `docs` and `semver` shipped since and need a
   place in this list ([audit][audit]).
4. **BREAKING:** two workflows replace three, and the proposal branch becomes
   `verctl/release/<base>`. Partly shipped: the branch and one reusable
   `release.yml` are on main; the three actions remain ([audit][audit]).

## Capabilities

### New Capabilities

- `release-state`: the facts, the release commit, the two stage axes, the
  transitions with their guards and postconditions, and how a rerun resumes.

### Modified Capabilities

None.

## Decisions for the operator

The operator approved these as decisions 1 and 2 of
[release-state-machine][rsm] on 2026-10-12.

1. Derive state from facts on every run, with no state file or labels.
   Partly settled: `publish` derives each version commit from first-parent
   history, and no label is read.
2. Release on push to the release branch, then propose. Settled: the
   reusable `release.yml` publishes, then opens the Version PR, on push.

## Impact

1. Code: `src/publish.rs`, `src/release.rs`, `src/git.rs`, `src/github.rs`,
   and `src/cli.rs`.
2. Actions: the three actions shrink to what the two workflows call.
   Queue row [VER-043][ver-043] tracks moving them to their own repositories.
3. Identity: `propose` needs a token whose pull request events start CI without
   approval.
   The decision on whether that is a dedicated GitHub App sits in queue row
   [VER-044][ver-044].

[rsm]: ../release-state-machine/proposal.md
[research]: ../release-state-machine/research.md
[audit]: ../release-state-machine/audit.md
[ver-011]: ../../../tasks.yaml
[ver-043]: ../../../tasks.yaml
[ver-044]: ../../../tasks.yaml

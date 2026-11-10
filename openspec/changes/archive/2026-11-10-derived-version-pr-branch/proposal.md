# Derive the Version PR branch from the base branch

## Motivation

The Version PR branch is the constant `version-packages`, the name
Changesets uses, though verctl serves one Version PR per base branch.
Auto-merge is `auto-merge.sh`, which a reusable workflow cannot run without a
second checkout of verctl.

The operator ruled on 2026-10-15 that the minimal slice B of
[release-state-machine][rsm] is unpaused.
This part is its proposal branch `verctl/release/<base>`, which
[`version-pr-identity`][vpi] defines.
Queue row: [VER-066][ver-066].
This change was split from [reusable-release-workflow][rrw] at its capability
boundaries.
It ships as a patch.

## Changes

1. The Version PR branch becomes `verctl/release/<base>`, and every reader of
   the branch follows it.
2. `prepare --pr --auto-merge` enables auto-merge from Rust, so the workflow
   and the action share one implementation, and `auto-merge.sh` goes.

## Capabilities

### New Capabilities

- `version-pr-branch`: the derived branch and what reads it.

### Modified Capabilities

None. The `version-pr-auto-merge` change keeps its requirements; its script
moves into `prepare --pr`.

## Impact

1. Code: `src/release.rs`, `src/versions.rs`, and `src/github.rs`.
2. Actions: `actions/version-pr` calls `prepare --pr --auto-merge`;
   `auto-merge.sh` is deleted. The actions stay for hand-written workflows
   until [VER-043][ver-043].

[rsm]: ../../release-state-machine/proposal.md
[rrw]: ../../reusable-release-workflow/proposal.md
[ver-043]: ../../../../tasks.yaml
[ver-066]: ../../../../tasks.yaml
[vpi]: ../../version-pr-identity/proposal.md

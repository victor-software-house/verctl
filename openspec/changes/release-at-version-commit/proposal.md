# Publish at the commit that set each version

## Why

`verctl publish` decides a release from the commit it runs on and creates the
tag and the Release at HEAD. A rerun after later commits land can tag a commit
that was never published. A run that finds nothing to do has no way to
report that.

The operator ruled on 2026-10-15 that the minimal slice B of
[release-state-machine][rsm] is unpaused.
This part is its release commit from first-parent history.
Queue row: [VER-066][ver-066].
This change was split from [reusable-release-workflow][rrw] at its capability
boundaries.
It ships as a patch.

## What Changes

1. `verctl publish` acts on the commit that set each version, found on the
   default branch's first-parent history, never on a later HEAD.
2. `verctl publish --plan` lists every incomplete version commit, oldest
   first, so one workflow run releases all of them in order.
3. `verctl publish` checks the tag and the Release before it creates them.
   When everything already holds it reports `nothing to release` and exits
   zero.

## Capabilities

### New Capabilities

- `idempotent-publish`: the version commit, which packages a publish acts on,
  and `nothing to release`.

### Modified Capabilities

None.

## Impact

1. Code: `src/publish.rs`, `src/versions.rs`, `src/git.rs`, and
   `src/github.rs`.
2. Workflow: the `plan` job reads `commits`, `pending`, and `assets` from
   `verctl publish --plan`, as [reusable-release-workflow][rrw] describes.

[rsm]: ../release-state-machine/proposal.md
[rrw]: ../reusable-release-workflow/proposal.md
[ver-066]: ../../../tasks.yaml

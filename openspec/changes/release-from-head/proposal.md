# Release from HEAD

## Motivation

A release across several packages cannot be atomic on any registry verctl publishes to.
verctl releases are already idempotent, but a failed release is retried at its old version commit, where the environment breaks with time.

shrub 0.7.0 shows the cost ([run 40251818311][shrub-070]):

1. The run published `shrub-parser` 0.7.0, then crates.io refused `shrub-process`, a new crate, because trusted publishing cannot create a crate.
2. One of 9 crates is live, and the other 8 have stayed at 0.6.2 or unpublished since 2026-10-26.
3. Each later push retried at the version commit `1ecc0b5`, and from 2026-11-11 each retry failed at `mise install --locked` on that commit's old lock.
4. `refuse_skipped_versions` blocks every newer version, so shrub releases nothing.
5. `main` has since moved 24 commits past the version commit, with 3 fragments pending.

The research note `docs/research/2026-11-13-atomic-releases.md` holds the analysis, the registry limits, and the operator's decisions of 2026-11-13.
This change is the first of four that carry them out:

1. `release-from-head`, this change;
2. [`release-order`][release-order]: the tag comes last;
3. [`release-checks`][release-checks]: every upload is proven before the first;
4. [`first-publish`][first-publish]: a package the registry lacks.

Queue row: [VER-129][ver-129].

## Changes

1. **HEAD is the release reference.**
   A run releases the versions HEAD's manifests hold, from HEAD, whichever commit set them.
2. **The old-commit path goes.**
   The oldest-first plan of every incomplete version commit, the jobs at old commits, the refusal to release anywhere but a version commit, and `refuse_skipped_versions` go.
3. **A version HEAD no longer holds stays unreleased.**
   Releasing it later is a separate opt-in that no change adds yet.
4. **`release.yml` checks out one commit**, the one that triggered it.

This replaces the earlier draft of this change, which installed only the release tools at an old version commit.
No run checks out an old commit any more.

## Release

A breaking patch, through a `patch!` fragment listed under `### Breaking` (operator, 2026-11-13).
A version HEAD no longer holds is no longer released, `status --github-output` writes HEAD as the only commit, and a release no longer refuses a skipped version.

[shrub-070]: https://github.com/victor-software-house/shrub/actions/runs/40251818311
[release-order]: ../release-order/proposal.md
[release-checks]: ../release-checks/proposal.md
[first-publish]: ../first-publish/proposal.md
[ver-129]: ../../../tasks.yaml

# Releases that land whole

## Motivation

A release across several packages cannot be atomic on any registry verctl publishes to.
verctl releases are already idempotent, but nothing proves, before the first upload, that every upload can succeed.
A version bump can also land on the default branch long before its release completes.

shrub 0.7.0 shows the cost ([run 40251818311][shrub-070]):

1. The run published `shrub-parser` 0.7.0, then crates.io refused `shrub-process`, a new crate, because trusted publishing cannot create a crate.
2. One of 9 crates is live, and the other 8 have stayed at 0.6.2 or unpublished since 2026-10-26.
3. Each later push retried at the version commit `1ecc0b5`, and from 2026-11-11 each retry failed at `mise install --locked` on that commit's old lock.
4. `refuse_skipped_versions` blocks every newer version, so shrub releases nothing.
5. `main` has since moved 24 commits past the version commit, with 3 fragments pending.

The first failure was knowable before any upload.
The research note `docs/research/2026-11-13-atomic-releases.md` holds the analysis, the registry limits, and the operator's decisions of 2026-11-13.

Queue row: [VER-129][ver-129].

## Changes

1. **Checks.**
   `verctl check --registry` proves, on every pull request, that each crate exists, lists the repository's workflow as its trusted publisher, and has the publishing identity among its owners.
   `verctl check --release` adds `cargo publish --workspace --dry-run`, the replay of the Version PR, and every asset built into a draft Release.
2. **HEAD is the release reference.**
   A run releases the versions HEAD's manifests hold, from HEAD.
   The oldest-first plan of every incomplete version commit, the jobs at old commits, and `refuse_skipped_versions` go.
3. **Order.**
   Assets go into a draft Release first, then the registries in dependency order, then the draft is published, which creates the tag.
4. **The freeze.**
   While the default branch holds a version whose release is incomplete, `check --registry` fails every other pull request or merge group.
   A pull request labelled `verctl: unfreeze` passes.
5. **A partial release** is a state `status` reports, with its cause, and a rerun resumes it.
6. **First publish.**
   `verctl release --first` publishes, with the operator's token, the real code of each package the registry lacks, at the version the default branch holds, tags it, and runs `verctl trust`.
   `verctl trust` registers or checks the trusted publisher of each crate.
   A repository may opt in to a token in CI for first publishes.
7. **Enforcement tiers.**
   `status` names the tier the default branch's rules give: a merge queue, rulesets without a queue, or neither.

This replaces the earlier draft of this change, which installed only the release tools at an old version commit.
No run checks out an old commit any more.

[shrub-070]: https://github.com/victor-software-house/shrub/actions/runs/40251818311
[ver-129]: ../../../tasks.yaml

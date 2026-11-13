# Release checks

## Motivation

verctl releases are idempotent, but nothing proves, before the first upload, that every upload can succeed.
A version bump can also land on the default branch long before its release completes.

shrub 0.7.0 published one crate of 9, then crates.io refused a new crate, a failure knowable before any upload.
While its release stayed incomplete, `main` took 24 more commits.

`check` also mixes two models today: bare `check` validates fragments and served files, and `check --versions` replaces that run instead of adding to it.

The research note `docs/research/2026-11-13-atomic-releases.md` holds the analysis and the operator's decisions of 2026-11-13.
This change follows [`release-order`][release-order].

Queue row: [VER-135][ver-135].

## Changes

1. **Named checks.**
   `verctl check [CHECK]...` runs `fragments`, `served`, `versions`, `registry` and `trust`, as `cargo deny check` runs its named checks.
   Bare `check` runs `fragments` and `served`, offline; `--all` (`-a`) runs every check.
   `check --versions` is removed, with no hidden flag.
   The hidden renamed verbs `prepare`, `pin`, `publish` and `assets`, and the hidden `release --plan`, go too; each then fails as an unknown command.
2. **The `registry` check** needs no token: each crate exists, has its expected owners, and its latest version shows whether it came through trusted publishing.
   The `trust` check needs an owner's token and proves the trusted publisher entry.
3. **The `owners` key** lists the expected owners once, with a per-package override.
4. **`release`** runs the registry checks `release_checks` names before its first upload, and `release --dry-run` prints the plan and runs the same checks; nothing another gate already proves runs again.
5. **The freeze.**
   While the default branch holds a version whose release is incomplete, the `registry` check fails every other pull request or merge group.
   A pull request labelled `verctl: unfreeze` passes.
6. **Enforcement tiers.**
   `status` names the tier the default branch's rules give: a merge queue, rulesets without a queue, or neither.

## Release

A breaking patch, through a `patch!` fragment listed under `### Breaking` (operator, 2026-11-13): `check --versions`, the hidden verbs and `release --plan` go, and a check that passed may now fail.
Streaming progress and diagnostics are queue row [VER-138][ver-138], outside this change.

[release-order]: ../release-order/proposal.md
[ver-135]: ../../../tasks.yaml
[ver-138]: ../../../tasks.yaml

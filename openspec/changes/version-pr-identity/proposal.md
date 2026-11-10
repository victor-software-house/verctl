# Identify the Version PR by facts that cannot change

## Why

The Version PR's own CI failed the hand-edit guard on the v0.1.1 release,
because the guard reads a label from an event snapshot taken before the label
existed (VER-029). The fix was a second CI trigger.

Queue row: VER-011. This change is one part of
[release-state-machine][rsm]; what a Version PR may contain is
[version-pr-content][vpc].

## What Changes

1. A Version PR is identified by its head repository and its head branch
   `verctl/release/<base>`, which a pull request cannot change, and by its
   base, read live on every run. Title, body, and labels are never read.
2. One Version PR per base. `propose` finds, opens, updates, and closes it,
   and refuses while a pull request on the proposal branch has another base.

## Capabilities

### New Capabilities

- `version-proposal`: Version PR identity, its lifecycle, and the rule
  `check` applies.

### Modified Capabilities

None.

## Decisions for the operator

The operator approved these as decisions 3 and 4 of
[release-state-machine][rsm] on 2026-10-12.

3. Identify the Version PR by head repository, head branch, and base, and
   verify its content too (design 2). Identity settled by [#188][pr188]; the
   content check is not built.
4. One Version PR per base on `verctl/release/<base>`; no per-package PRs and
   no version in the branch name (design 3). Settled by [#161][pr161].

## Impact

1. Code: `src/release.rs`, `src/github.rs`, and `src/versions.rs`.
2. CI: the `labeled` and `unlabeled` trigger types go.

[rsm]: ../release-state-machine/proposal.md
[vpc]: ../version-pr-content/proposal.md
[pr161]: https://github.com/victor-software-house/verctl/pull/161
[pr188]: https://github.com/victor-software-house/verctl/pull/188

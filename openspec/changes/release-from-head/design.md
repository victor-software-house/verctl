# Design

The research note `docs/research/2026-11-13-atomic-releases.md` holds the evidence for each decision.
The operator made each decision on 2026-11-13.

The change covers every kind of package that verctl releases.
npm and GitHub Packages, Go modules, and other registries are horizon rows of the [work queue][queue] for the later changes.

## 1. HEAD is the release reference

**Decision.**
A run releases the versions HEAD's manifests hold, from HEAD.
It never releases an older version, before a newer one or in the same run.
`status` lists HEAD's incomplete releases only.

**Alternative.**
Release every incomplete version commit oldest first, each at its own commit, as today.
It lost because an old commit's environment breaks with time, as shrub's lock did, and because a retry then ships code the Version PR never covered.

**Override.**
A later, separate opt-in may release a skipped version; this change adds none.

## 2. A release runs at any commit that holds an incomplete version

**Decision.**
`verctl release` no longer refuses a commit that is not a version commit.
At a later commit it releases the versions HEAD still holds, with HEAD's code.

**Alternative.**
Keep the refusal and name the version commit, as today.
It lost because it sends the retry to the old commit, which decision 1 removes.
[`release-checks`][release-checks] adds the freeze, so the default branch takes no other change while a release is incomplete, and HEAD then stays at the version commit in practice.

**Override.**
None needed: the proofs that the default branch contains HEAD and that its workflows match keep their `--allow-…` flags.

## 3. One checkout per run

**Decision.**
The reusable `release.yml` checks out `github.sha` in every job, and has no matrix of version commits.

**Alternative.**
Keep the matrix and install only the release tools at the old commit, as the first draft of this change proposed.
It lost to decision 1: no job needs an old commit.

**Disable.**
None: a caller that needs another commit dispatches the workflow at that commit.

[queue]: ../../../tasks.yaml
[release-checks]: ../release-checks/proposal.md

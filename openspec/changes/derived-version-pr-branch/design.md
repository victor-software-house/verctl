# Design

## Context

The Version PR branch is the constant `version-packages`, the name Changesets
uses.
verctl serves one Version PR per base branch.
Auto-merge is a shell script that the workflow and the action each carry.
This change derives the branch and moves auto-merge into Rust.
It is one of the parts
[reusable-release-workflow][rrw] was split into.

## 1. The Version PR branch

`prepare --pr` commits on `verctl/release/<base>`, where `<base>` is the
branch the Version PR targets.
`check --versions` exempts any branch named `verctl/release/<base>` locally.
In CI it exempts a pull request that meets all of these
([`version-pr-identity`][vpi] section 1):

- the head branch is `verctl/release/<base>`;
- the head repository is this repository;
- the current base is `<base>`.

No configuration names the branch.

Alternative: a configurable branch name. Lost: the name identifies the Version
PR, and [`version-pr-identity`][vpi] section 3 already chose the derived
form.

## 2. Auto-merge in Rust

`prepare --pr --auto-merge` reads the base branch's rules and refuses when none
requires a status check.
It enables auto-merge as a squash titled `<title> (#<number>)` with the PR
description as body.
This is the behaviour of `auto-merge.sh` today.

The workflow passes the job's own token as `VERCTL_JOB_TOKEN`.
verctl refuses auto-merge when the token it holds is that one, because a merge
made with it starts no workflow.

Alternative: keep `auto-merge.sh` and call it from the reusable workflow.
Lost: a called workflow cannot run a script from verctl's repository without a
second checkout of verctl at the pinned commit.
The action and the workflow would also carry two copies of the same rule.

[rrw]: ../reusable-release-workflow/design.md
[vpi]: ../version-pr-identity/design.md

# Design

## Context

The Version PR branch is the constant `version-packages`, the name Changesets
uses, though verctl serves one Version PR per base branch, and auto-merge is a
shell script the workflow and the action each carry. This change derives the
branch and moves auto-merge into Rust. It is one of the parts
[reusable-release-workflow][rrw] was split into.

## 1. The Version PR branch

`prepare --pr` commits on `verctl/release/<base>`, where `<base>` is the
branch the Version PR targets. `check --versions` exempts any branch named
`verctl/release/<base>` locally, and in CI a pull request with that head
branch, this repository as its head repository, and `<base>` as its current
base ([`version-pr-identity`][vpi] section 1). The branch is derived, not
configured.

Alternative: a configurable branch name. Lost: the name identifies the Version
PR, and [`version-pr-identity`][vpi] section 3 already chose the derived
form.

## 2. Auto-merge in Rust

`prepare --pr --auto-merge` reads the base branch's rules, refuses when none
requires a status check, and enables auto-merge as a squash titled
`<title> (#<number>)` with the PR description as body, the behaviour of
`auto-merge.sh` today. The workflow passes the job's own token as
`VERCTL_JOB_TOKEN`, and verctl refuses auto-merge when the token it holds is
that one, because a merge made with it starts no workflow.

Alternative: keep `auto-merge.sh` and call it from the reusable workflow.
Lost: a called workflow cannot run a script from verctl's repository without a
second checkout of verctl at the pinned commit, and the action and the
workflow would carry two copies of the same rule.

[rrw]: ../reusable-release-workflow/design.md
[vpi]: ../version-pr-identity/design.md

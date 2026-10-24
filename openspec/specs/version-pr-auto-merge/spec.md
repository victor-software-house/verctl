# version-pr-auto-merge Specification

## Purpose

How a repository opts in to merging its Version PR once required checks pass.

## Requirements

### Requirement: The Version PR merges itself when a repository opts in

With `auto-merge: true`, `actions/version-pr` SHALL enable GitHub auto-merge
on the open Version PR after opening or updating it, as a squash whose
subject is the PR title followed by a space and `(#<number>)` and whose body
is the PR description. With `auto-merge` unset or `false`, it SHALL not change the
Version PR's merge state.

#### Scenario: Opted in with a required check

- **WHEN** the workflow runs the action with `auto-merge: true`, the base branch `main` requires the status check `Verify`, and the action opens Version PR #42 titled `chore(release): version packages`
- **THEN** Version PR #42 has auto-merge enabled with the squash subject `chore(release): version packages (#42)`

#### Scenario: Not opted in

- **WHEN** the workflow runs the action without `auto-merge`
- **THEN** the Version PR's auto-merge stays disabled

### Requirement: Auto-merge needs a required check on the base branch

With `auto-merge: true`, the action SHALL exit non-zero before enabling
auto-merge when the base branch has no required status check, and SHALL name
the branch.

#### Scenario: No required check

- **WHEN** the workflow runs the action with `auto-merge: true` and `main` has no `required_status_checks` rule
- **THEN** the action exits non-zero, names `main`, and the Version PR's auto-merge stays disabled

### Requirement: Auto-merge needs a token other than GITHUB_TOKEN

With `auto-merge: true`, the action SHALL exit non-zero before enabling
auto-merge when `token` is the workflow's `GITHUB_TOKEN`, because a merge made
with that token starts no publish workflow.

#### Scenario: Default token

- **WHEN** the workflow runs the action with `auto-merge: true` and no `token` input
- **THEN** the action exits non-zero, names `GITHUB_TOKEN`, and the Version PR's auto-merge stays disabled

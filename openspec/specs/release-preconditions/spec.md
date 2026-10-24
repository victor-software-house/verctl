# release-preconditions Specification

## Purpose

What a release needs from the clone and the workflow tree, and how a failure to meet it is reported.

## Requirements

### Requirement: A shallow clone fetches the default branch history

When `VERCTL_DEFAULT_BRANCH` names a branch and the clone is shallow,
`verctl publish` and `verctl publish --plan` SHALL fetch the whole history of
that branch from `origin` before walking history. A clone that is not
shallow SHALL NOT be fetched. Without `VERCTL_DEFAULT_BRANCH`, a walk that
reaches the shallow boundary SHALL fail and name the variable.

#### Scenario: A depth-1 checkout of a later commit

- **WHEN** the clone is `git clone --depth 1` of a branch whose version commit is an ancestor of HEAD, and `VERCTL_DEFAULT_BRANCH` names that branch
- **THEN** `verctl publish` finds the version commit

#### Scenario: No default branch configured

- **WHEN** the same clone runs `verctl publish` without `VERCTL_DEFAULT_BRANCH`
- **THEN** it fails, says the clone is shallow, and names `VERCTL_DEFAULT_BRANCH`

### Requirement: A refused Release names the missing permission

When creating a GitHub Release fails with status 403, `verctl publish` SHALL
fail with a message that names the tag, the commit, `workflows: write`, and
that `GITHUB_TOKEN` cannot hold it. The status SHALL be read from the typed
error, not from the message text.

#### Scenario: Resource not accessible by integration

- **WHEN** GitHub answers the create request with 403 "Resource not accessible by integration"
- **THEN** the error names `workflows: write` and the commit

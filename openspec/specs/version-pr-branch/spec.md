# version-pr-branch Specification

## Purpose

How the Version PR branch name derives from its base branch, and how the code that reads the branch finds it.

## Requirements

### Requirement: The Version PR branch is derived from the base branch

`verctl prepare --pr` SHALL commit the version step on `verctl/release/<base>`,
where `<base>` is the branch the Version PR targets, and SHALL open or update
the pull request from that branch. No configuration SHALL name the branch.

#### Scenario: A repository on main

- **WHEN** `prepare --pr` runs on push to `main` with a pending fragment
- **THEN** it force-updates `verctl/release/main` and opens or updates the pull request from it into `main`

#### Scenario: A repository on trunk

- **WHEN** the default branch is `trunk` and `GITHUB_REF_NAME` is `trunk`
- **THEN** the Version PR's head branch is `verctl/release/trunk`

### Requirement: Readers of the branch follow the new name

`verctl check --versions` SHALL exempt a local branch named
`verctl/release/<base>` instead of `version-packages`.
In CI it SHALL exempt a pull request, read through the API, that meets all of
these:

- its head repository is the repository;
- its head branch is `verctl/release/<base>`;
- its current base is `<base>`.

#### Scenario: Checking versions on the Version PR branch

- **WHEN** `verctl check --versions` runs locally on branch `verctl/release/main` with bumped manifests
- **THEN** it exits zero

#### Scenario: Checking versions on the old branch

- **WHEN** `verctl check --versions` runs locally on a branch named `version-packages` with bumped manifests
- **THEN** it exits non-zero and names the manifests

## MODIFIED Requirements

### Requirement: The Version PR branch is derived from the base branch

`verctl version --pr` SHALL commit the version step on `verctl/release/<base>`,
where `<base>` is the branch the Version PR targets, and SHALL open or update
the pull request from that branch. No configuration SHALL name the branch.

#### Scenario: A repository on main

- **WHEN** `verctl version --pr` runs on push to `main` with a pending fragment
- **THEN** it force-updates `verctl/release/main` and opens or updates the pull request from it into `main`

#### Scenario: A repository on trunk

- **WHEN** the default branch is `trunk` and `GITHUB_REF_NAME` is `trunk`
- **THEN** the Version PR's head branch is `verctl/release/trunk`

## ADDED Requirements

### Requirement: The version check fetches the branches it compares against

Before comparing versions, `verctl check --versions` SHALL fetch
`origin/<branch>` with its whole history for each of `VERCTL_DEFAULT_BRANCH`
and `GITHUB_BASE_REF` that is set, when that ref is missing or the clone is
shallow. In a shallow clone it SHALL undo the boundary, so HEAD's parents are
fetched too. It SHALL fetch nothing when the clone is not shallow and the ref
exists, or when neither variable is set.

#### Scenario: A depth-1 pull request merge checkout

- **WHEN** a clone holds only a depth-1 fetch of `refs/pull/1/merge`, has no `origin/main`, `main` has moved on since the merge ref was made, `GITHUB_BASE_REF=main`, and `verctl check --versions` runs
- **THEN** `origin/main` is fetched and each manifest is compared with its copy at the merge-base of HEAD and `origin/main`, which is HEAD's first parent

#### Scenario: A full clone

- **WHEN** a clone that is not shallow has `origin/main`, `GITHUB_BASE_REF=main`, and `verctl check --versions` runs
- **THEN** nothing is fetched

#### Scenario: A local run

- **WHEN** `verctl check --versions` runs with neither `VERCTL_DEFAULT_BRANCH` nor `GITHUB_BASE_REF` set
- **THEN** nothing is fetched

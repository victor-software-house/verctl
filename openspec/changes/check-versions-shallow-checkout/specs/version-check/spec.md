## ADDED Requirements

### Requirement: The version check fetches the branches it compares against

Before comparing versions, `verctl check --versions` SHALL fetch
`origin/<branch>` with its whole history for each of `VERCTL_DEFAULT_BRANCH`,
the event payload's `repository.default_branch`, and `GITHUB_BASE_REF` that
is set, when that ref is missing or the clone is
shallow. In a shallow clone it SHALL undo the boundary, so HEAD's parents are
fetched too. It SHALL fetch nothing when the clone is not shallow and the ref
exists, or when none of them is set.
The released baseline SHALL be read from the first of
`VERCTL_DEFAULT_BRANCH`, the event's default branch, `main`, `master`, and
`GITHUB_BASE_REF` that `origin` has.

#### Scenario: A depth-1 pull request merge checkout

- **WHEN** a clone holds only a depth-1 fetch of `refs/pull/1/merge`, has no `origin/main`, `main` has moved on since the merge ref was made, `GITHUB_BASE_REF=main`, and `verctl check --versions` runs
- **THEN** `origin/main` is fetched and each manifest is compared with its copy at the merge-base of HEAD and `origin/main`, which is HEAD's first parent

#### Scenario: A pull request to another branch

- **WHEN** a depth-1 merge checkout of a pull request to `develop` runs `verctl check --versions` with `GITHUB_BASE_REF=develop`, an event payload whose `repository.default_branch` is `main`, and `VERCTL_DEFAULT_BRANCH` unset
- **THEN** both `origin/develop` and `origin/main` are fetched, so the released baseline reads `main` as in a full clone

#### Scenario: A default branch named trunk

- **WHEN** a depth-1 merge checkout of a pull request to `develop` runs `verctl check --versions` with `GITHUB_BASE_REF=develop`, an event payload whose `repository.default_branch` is `trunk`, and `VERCTL_DEFAULT_BRANCH` unset
- **THEN** `origin/trunk` is fetched and the released baseline is the merge-base of HEAD and `origin/trunk`, not `origin/develop`

#### Scenario: A full clone

- **WHEN** a clone that is not shallow has `origin/main`, `GITHUB_BASE_REF=main`, and `verctl check --versions` runs
- **THEN** nothing is fetched

#### Scenario: A local run

- **WHEN** `verctl check --versions` runs with no event payload and neither `VERCTL_DEFAULT_BRANCH` nor `GITHUB_BASE_REF` set
- **THEN** nothing is fetched

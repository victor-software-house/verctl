## MODIFIED Requirements

### Requirement: Readers of the branch follow the new name

`verctl check versions` SHALL exempt a local branch named
`verctl/release/<base>` instead of `version-packages`.
In CI it SHALL exempt a pull request, read through the API, that meets all of
these:

- its head repository is the repository;
- its head branch is `verctl/release/<base>`;
- its current base is `<base>`.

#### Scenario: Checking versions on the Version PR branch

- **WHEN** `verctl check versions` runs locally on branch `verctl/release/main` with bumped manifests
- **THEN** it exits zero

#### Scenario: Checking versions on the old branch

- **WHEN** `verctl check versions` runs locally on a branch named `version-packages` with bumped manifests
- **THEN** it exits non-zero and names the manifests

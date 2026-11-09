## ADDED Requirements

### Requirement: A tag on a merged commit at the same version names the release

A tag that names a commit other than the version commit SHALL name the release commit when HEAD contains that commit, on or off the first-parent history, and the package's version there equals its version at HEAD.
Trees SHALL NOT be compared.
Any other tagged commit SHALL fail the plan, naming the tag, the commit, and the version commit.

#### Scenario: A forkctl append publish keeps the tagged commit on a second parent

- **WHEN** tag `v0.0.1` names commit `T` with `demo` at `0.0.1`, a rewrite of the branch made commit `S` at `0.0.1` that changes `.github/workflows/publish.yml`, merge commit `M` has first parent `S` and second parent `T`, HEAD is `M`, and `demo@0.0.1` is on crates.io with its Release
- **THEN** `verctl publish --plan` treats `T` as the release commit and reports `nothing to release`

#### Scenario: The merged commit carries another version

- **WHEN** HEAD is merge commit `M` with `demo` at `0.0.1`, and tag `v0.0.1` names its second parent `T`, where `demo` is `0.0.2`
- **THEN** `verctl publish --plan` exits non-zero and names the tag, `T`, and the version commit

#### Scenario: HEAD does not contain the tagged commit

- **WHEN** tag `v0.0.1` names commit `T` at `0.0.1` on a branch that was rewritten and never merged back, and HEAD is the rewritten commit
- **THEN** `verctl publish --plan` exits non-zero and names the tag, `T`, and the version commit

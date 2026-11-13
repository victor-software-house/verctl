## MODIFIED Requirements

### Requirement: A new package releases 0.0.1 first

A new standalone package SHALL first publish at `0.0.0` through `verctl release --new`.
Its first Version PR release SHALL be `0.0.1`.

verctl SHALL refuse a `minor` or `major` bump on a package whose current version
is `0.0.0`, with or without a pre-release or build suffix.
A package that declares `first_minor: true` is the one exception.
The refusal SHALL name the package and the `first_minor` key.

`verctl check` SHALL fail a standalone package that was never published and whose version is not `0.0.0`.
A member of a shared-version group, a workspace member with `version.workspace = true`, SHALL first publish at the group's current version, and `check` SHALL NOT apply the `0.0.0` rule to it.

#### Scenario: A patch fragment on a new package

- **WHEN** `Cargo.toml` declares version `0.0.0` for package `demo` and `.changeset/first.md` holds `demo: patch`
- **THEN** `verctl version --dry-run` plans `demo` from `0.0.0` to `0.0.1`

#### Scenario: A minor fragment on a new package

- **WHEN** `Cargo.toml` declares version `0.0.0` for package `demo` and `.changeset/first.md` holds `demo: minor`
- **THEN** `verctl version --dry-run` exits non-zero
- **AND** its error names `package demo` and `first_minor`

#### Scenario: The operator opens a package at 0.1.0

- **WHEN** package `demo` at `0.0.0` declares `first_minor: true` in `.ctl/ver.yaml` and `.changeset/first.md` holds `demo: minor`
- **THEN** `verctl version --dry-run` plans `demo` from `0.0.0` to `0.1.0`

#### Scenario: A none fragment on a new package

- **WHEN** package `demo` at `0.0.0` has only `.changeset/first.md` holding `demo: none`
- **THEN** `verctl version --dry-run` plans no bump and exits zero

#### Scenario: A new standalone package first publishes at 0.0.0

- **WHEN** package `demo` at `0.0.0` is absent from crates.io and `verctl release --new` runs
- **THEN** it publishes `demo` `0.0.0`
- **AND** the first Version PR for `demo`, with `.changeset/first.md` holding `demo: patch`, plans `0.0.0` to `0.0.1`

#### Scenario: A never-published package at another version

- **WHEN** standalone package `demo` is absent from crates.io and `Cargo.toml` declares version `0.2.0`
- **THEN** `verctl check` exits non-zero and its error names `demo`, `0.2.0`, and `0.0.0`

#### Scenario: A member of a shared-version group

- **WHEN** workspace member `demo-cli` sets `version.workspace = true`, the workspace version is `0.7.0`, and `demo-cli` is absent from crates.io
- **THEN** `verctl release --new` publishes `demo-cli` `0.7.0`
- **AND** `verctl check` exits zero for `demo-cli`

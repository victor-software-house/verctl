# Versioning

## ADDED Requirements

### Requirement: A new package releases 0.0.1 first

verctl SHALL refuse a `minor` or `major` bump on a package whose current version
is exactly `0.0.0`, unless the package declares `first_minor: true`, and the
refusal SHALL name the package and the `first_minor` key.

#### Scenario: A patch fragment on a new package

- **WHEN** `Cargo.toml` declares version `0.0.0` for package `demo` and `.changeset/first.md` holds `demo: patch`
- **THEN** `verctl prepare --dry-run` plans `demo` from `0.0.0` to `0.0.1`

#### Scenario: A minor fragment on a new package

- **WHEN** `Cargo.toml` declares version `0.0.0` for package `demo` and `.changeset/first.md` holds `demo: minor`
- **THEN** `verctl prepare --dry-run` exits non-zero
- **AND** its error names `package demo` and `first_minor`

#### Scenario: The operator opens a package at 0.1.0

- **WHEN** package `demo` at `0.0.0` declares `first_minor: true` in `.ctl/ver.yaml` and `.changeset/first.md` holds `demo: minor`
- **THEN** `verctl prepare --dry-run` plans `demo` from `0.0.0` to `0.1.0`

#### Scenario: A none fragment on a new package

- **WHEN** package `demo` at `0.0.0` has only `.changeset/first.md` holding `demo: none`
- **THEN** `verctl prepare --dry-run` plans no bump and exits zero

### Requirement: check refuses what prepare refuses

`verctl check` SHALL plan the pending fragments against `.ctl/ver.yaml` and the
manifests it names, and SHALL fail on any bump `prepare` would refuse.

#### Scenario: A minor fragment on a new package fails check

- **WHEN** package `demo` at `0.0.0` has `.changeset/first.md` holding `demo: minor`
- **THEN** `verctl check` exits non-zero and its error names `first_minor`

#### Scenario: A patch fragment on a new package passes check

- **WHEN** package `demo` at `0.0.0` has `.changeset/first.md` holding `demo: patch`
- **THEN** `verctl check` exits zero

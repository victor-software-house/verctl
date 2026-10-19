## ADDED Requirements

### Requirement: A patch release is checked for API breaks

`verctl semver` SHALL run `cargo semver-checks --release-type minor`
against the crates.io baseline for each crates.io crate whose package has no
pending `minor` or `major` fragment and whose manifest major.minor equals the
baseline's, and SHALL fail naming each crate whose check fails.

#### Scenario: A break on a 0.0.z crate

- **WHEN** `demo` is at 0.0.2 on crates.io and in its manifest, `.changeset/` holds only `demo: patch`, and a public function was removed
- **THEN** `verctl semver` runs `cargo semver-checks --manifest-path Cargo.toml --package demo --baseline-version 0.0.2 --release-type minor`
- **AND** it exits non-zero and names `demo@0.0.2`

#### Scenario: An addition on a 0.y.z crate

- **WHEN** `demo` is at 0.4.10 and a public function was added
- **THEN** `verctl semver` reports `demo` as `passed`

### Requirement: A minor release skips the check

`verctl semver` SHALL skip a crate whose package has a pending `minor` or
`major` fragment, or whose manifest major.minor differs from its crates.io
baseline.

#### Scenario: A minor fragment

- **WHEN** `.changeset/break.md` names `demo: minor`
- **THEN** `verctl semver` reports `demo` as skipped and runs no check for it

#### Scenario: The Version PR

- **WHEN** the manifest says 0.1.0 and crates.io's newest release is 0.0.2
- **THEN** `verctl semver` reports `demo` as skipped

### Requirement: Each crate of a workspace package is checked

`verctl semver` SHALL check each publishing member of a virtual workspace
package under its own crate name, and SHALL skip a crate with no release on
crates.io.

#### Scenario: A new member

- **WHEN** the package at a virtual `Cargo.toml` has members `demo-core` (0.0.2 on crates.io) and `demo-cli` (never published)
- **THEN** `verctl semver` checks `demo-core` and reports `demo-cli` as `unpublished`

### Requirement: A crate with no library target is left out

`verctl semver` SHALL leave out a crates.io crate whose package has no
`lib`, `rlib`, `dylib`, `cdylib`, or `staticlib` target, the targets
cargo-semver-checks checks, and SHALL run no check for it.

#### Scenario: A binary member of a workspace package

- **WHEN** the package at a virtual `Cargo.toml` has members `demo-core` (a library, 0.0.2 on crates.io) and `demo` (only `src/main.rs`, 0.0.2 on crates.io)
- **THEN** `verctl semver` checks `demo-core`, runs no check for `demo`, and passes

### Requirement: A crate at 1.0 fails

`verctl semver` SHALL fail when a crates.io crate's manifest version is 1.0
or later, naming the crate and that the rule covers 0.x only.

#### Scenario: A 1.0 crate

- **WHEN** `demo` is at 1.0.0 in its manifest
- **THEN** `verctl semver` exits non-zero and its error names `demo` and 1.0

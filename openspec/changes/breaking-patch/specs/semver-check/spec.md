## ADDED Requirements

### Requirement: A `patch!` fragment skips the check for one release

`verctl semver` SHALL skip a crate whose package's largest pending bump is a patch when a pending fragment bumps that package by `patch!`.
It SHALL report the crate as `skipped: patch! fragment` and run no check for it.
A pending `minor` or `major` fragment SHALL keep its own decision.

On a Version PR, `verctl semver` SHALL also skip a crate one patch above its crates.io baseline when the CHANGELOG section of the manifest version holds `### Breaking`.

#### Scenario: A marked break

- **WHEN** `verctl` is at 0.8.0 on crates.io and in its manifest, `.changeset/library-api.md` sets `verctl: patch!`, and a public field was removed
- **THEN** `verctl semver` exits zero and reports `verctl` as `skipped: patch! fragment`

#### Scenario: The next release checks again

- **WHEN** the Version PR consumed `.changeset/library-api.md`, crates.io holds 0.8.1, and `.changeset/` holds only `verctl: patch`
- **THEN** `verctl semver` runs `cargo semver-checks` for `verctl` against 0.8.1

#### Scenario: The Version PR of a marked patch

- **WHEN** the Version PR consumed `.changeset/library-api.md`, `Cargo.toml` holds `version = "0.8.1"`, crates.io holds 0.8.0, and `CHANGELOG.md` has `## verctl 0.8.1` with `### Breaking`
- **THEN** `verctl semver` reports `verctl` as `skipped: patch! fragment`

#### Scenario: A minor beside the mark

- **WHEN** `.changeset/` holds `demo: patch!` and `demo: minor`
- **THEN** `verctl semver` reports `demo` as `skipped: minor fragment`

## ADDED Requirements

### Requirement: A package may be released on GitHub only

A package with `publisher: none` SHALL be published with no upload command.
`publish` SHALL still create its tag and GitHub Release.

#### Scenario: Private binary

- **WHEN** `.ctl/ver.yaml` declares `demo` at `Cargo.toml` with `publisher: none`, and the manifest sets `publish = false`
- **THEN** `verctl publish --dry-run` lists `demo` via `release only` and would create `v0.0.1`

#### Scenario: Rerun after the Release exists

- **WHEN** `publish` runs again for a release-only package whose Release already exists
- **THEN** it reports that Release and creates nothing

### Requirement: A release-only package names no registry

The load SHALL fail when a package sets `publisher: none` together with `registry` or `publish`.

#### Scenario: Registry beside none

- **WHEN** a package sets `publisher: none` and `registry: crates-io`
- **THEN** loading `.ctl/ver.yaml` fails with a message that names the package and says it has `publisher: none`

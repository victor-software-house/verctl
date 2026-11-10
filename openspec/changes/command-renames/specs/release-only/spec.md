## MODIFIED Requirements

### Requirement: A package may be released on GitHub only

A package with `publisher: none` SHALL be published with no upload command.
`release` SHALL still create its tag and GitHub Release.

#### Scenario: Private binary

- **WHEN** `.ctl/ver.yaml` declares `demo` at `Cargo.toml` with `publisher: none`, and the manifest sets `publish = false`
- **THEN** `verctl release --dry-run` lists `demo` via `release only` and would create `v0.0.1`

#### Scenario: Rerun after the Release exists

- **WHEN** `release` runs again for a release-only package whose Release already exists
- **THEN** it reports that Release and creates nothing

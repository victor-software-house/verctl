# release-only Specification

## Purpose

How a package is released on GitHub only, with no registry upload.

## Requirements

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

### Requirement: Only none uploads nothing

The load SHALL fail when `publishers` declares a `none` entry.
Resolving any other publisher to an empty upload command SHALL fail and name the package.

#### Scenario: Empty publish command

- **WHEN** a package sets `publish: []`
- **THEN** resolving its publisher fails with a message that says it renders an empty command and points at `publisher: none`

#### Scenario: A publishers.none entry

- **WHEN** `.ctl/ver.yaml` declares `publishers.none`
- **THEN** loading it fails and says the name is reserved

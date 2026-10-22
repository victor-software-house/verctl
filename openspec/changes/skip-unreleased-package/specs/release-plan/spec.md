# Release plan

## ADDED Requirements

### Requirement: A package at 0.0.0 is not planned

`verctl publish --plan` and `verctl publish` SHALL leave out every package whose version is `0.0.0`, with or without a pre-release or build suffix, and SHALL do so before any registry or GitHub lookup for it.
`verctl publish` SHALL NOT require a changelog section for such a package.

#### Scenario: A crate adopted at 0.0.0

- **WHEN** `.ctl/ver.yaml` declares only package `demo`, `Cargo.toml` declares version `0.0.0`, and `GITHUB_REPOSITORY` names the repository
- **THEN** `verctl publish --plan` reports `nothing to release` and writes `pending=false` to `$GITHUB_OUTPUT`

#### Scenario: No changelog section for 0.0.0

- **WHEN** `.ctl/ver.yaml` declares only package `demo` at `0.0.0` and no changelog exists
- **THEN** `verctl publish` does not fail for a missing changelog section

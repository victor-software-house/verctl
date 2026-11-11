# release-plan Specification

## Purpose

Which packages `verctl publish --plan` and `verctl publish` plan as releases.

## Requirements

### Requirement: A package at 0.0.0 is not planned

`verctl publish --plan` and `verctl publish` SHALL leave out every package whose version is `0.0.0`, with or without a pre-release or build suffix.
They SHALL do so before any registry or GitHub lookup for the package.
`verctl publish` SHALL NOT require a changelog section for such a package.

#### Scenario: A crate adopted at 0.0.0

- **WHEN** `.ctl/ver.yaml` declares only package `demo`, `Cargo.toml` declares version `0.0.0`, and `GITHUB_REPOSITORY` names the repository
- **THEN** `verctl publish --plan` reports `nothing to release` and writes `pending=false` to `$GITHUB_OUTPUT`

#### Scenario: No changelog section for 0.0.0

- **WHEN** `.ctl/ver.yaml` declares only package `demo` at `0.0.0` and no changelog exists
- **THEN** `verctl publish` does not fail for a missing changelog section

#### Scenario: A new package beside a released one under a shared tag

- **WHEN** `.ctl/ver.yaml` declares package `demo` at `1.0.0` and package `extra` at `0.0.0`, with the default `v{version}` tag template
- **THEN** `verctl publish --dry-run` lists only `demo` and would create `v1.0.0`

### Requirement: A tag on a merged commit at the same version names the release

A tag that names a commit other than the version commit SHALL name the release commit when both of these hold:

- HEAD contains that commit, on or off the first-parent history;
- the package's version there equals its version at HEAD.

Trees SHALL NOT be compared.
Any other tagged commit SHALL fail the plan, naming the tag, the commit, and the version commit.

#### Scenario: A forkctl append publish keeps the tagged commit on a second parent

- **WHEN** tag `v0.0.1` names commit `T` with `demo` at `0.0.1`
- **AND** a rewrite of the branch made commit `S` at `0.0.1` that changes `.github/workflows/publish.yml`
- **AND** merge commit `M` has first parent `S` and second parent `T`, and HEAD is `M`
- **AND** `demo@0.0.1` is on crates.io with its Release
- **THEN** `verctl publish --plan` treats `T` as the release commit and reports `nothing to release`

#### Scenario: Another version on the merged commit

- **WHEN** HEAD is merge commit `M` with `demo` at `0.0.1`, and tag `v0.0.1` names its second parent `T`, where `demo` is `0.0.2`
- **THEN** `verctl publish --plan` exits non-zero and names the tag, `T`, and the version commit

#### Scenario: HEAD does not contain the tagged commit

- **WHEN** tag `v0.0.1` names commit `T` at `0.0.1` on a branch that was rewritten and never merged back, and HEAD is the rewritten commit
- **THEN** `verctl publish --plan` exits non-zero and names the tag, `T`, and the version commit

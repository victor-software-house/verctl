## ADDED Requirements

### Requirement: A package starts candidates with `prerelease`

A package in `.ctl/ver.yaml` MAY set `prerelease` to a tag of lowercase ASCII letters.
Loading `.ctl/ver.yaml` SHALL fail on any other tag, naming the package and the field.
When `prerelease` is omitted, the package SHALL release stable versions.

#### Scenario: A tag of letters

- **WHEN** `.ctl/ver.yaml` declares package `verctl` with `prerelease: rc`, and `verctl check` runs
- **THEN** the config loads

#### Scenario: A tag with a digit

- **WHEN** `.ctl/ver.yaml` declares package `verctl` with `prerelease: rc1`, and `verctl check` runs
- **THEN** it exits non-zero, naming `verctl`, `prerelease`, and the lowercase letters a tag holds

### Requirement: A Version PR writes a candidate while `prerelease` is set

While a package sets `prerelease`, `verctl version` SHALL write `<base>-<tag>.<count>` for it.
The base SHALL be the current version bumped by the highest pending bump.
A current prerelease SHALL keep its base when the base already reaches that bump: a patch always, a minor when the patch is 0, and a major when the minor and patch are 0.
The count SHALL be the current count plus 1 when the base and the tag are unchanged, and 1 otherwise.

#### Scenario: The first candidate of a minor

- **WHEN** `Cargo.toml` holds `version = "0.7.4"`, the package sets `prerelease: rc`, and a fragment holds `verctl: minor`
- **THEN** `verctl version` writes `version = "0.8.0-rc.1"`

#### Scenario: A fix on top of a candidate

- **WHEN** `Cargo.toml` holds `version = "0.8.0-rc.1"`, the package sets `prerelease: rc`, and a fragment holds `verctl: patch`
- **THEN** `verctl version` writes `version = "0.8.0-rc.2"`

#### Scenario: A minor that the base already holds

- **WHEN** `Cargo.toml` holds `version = "0.8.0-rc.2"`, the package sets `prerelease: rc`, and a fragment holds `verctl: minor`
- **THEN** `verctl version` writes `version = "0.8.0-rc.3"`

#### Scenario: A minor above a patch candidate

- **WHEN** `Cargo.toml` holds `version = "1.2.4-rc.1"`, the package sets `prerelease: rc`, and a fragment holds `verctl: minor`
- **THEN** `verctl version` writes `version = "1.3.0-rc.1"`

### Requirement: A candidate version in another shape fails the plan

`verctl version` SHALL fail for a package whose current version has a prerelease that is not `<letters>.<number>`.
The error SHALL name the version and the shape verctl writes.

#### Scenario: A count with no dot

- **WHEN** `Cargo.toml` holds `version = "0.8.0-rc1"`, the package sets `prerelease: rc`, and a fragment holds `verctl: patch`
- **THEN** `verctl version` exits non-zero, naming `0.8.0-rc1` and `0.8.0-rc.2` as the shape it writes
- **AND** `Cargo.toml` is unchanged

### Requirement: A new tag starts its own count

A package whose current version is a prerelease with a tag other than its `prerelease` SHALL be pending with no fragment.
`verctl version` SHALL write the same base with the new tag and count 1.
A new tag that sorts below the current tag SHALL fail the plan, naming both versions.

#### Scenario: From beta to rc

- **WHEN** `Cargo.toml` holds `version = "0.8.0-beta.2"`, the package sets `prerelease: rc`, and no fragment is pending
- **THEN** `verctl version` writes `version = "0.8.0-rc.1"`

#### Scenario: Back from rc to beta

- **WHEN** `Cargo.toml` holds `version = "0.8.0-rc.2"`, the package sets `prerelease: beta`, and no fragment is pending
- **THEN** `verctl version` exits non-zero, naming `0.8.0-rc.2` and `0.8.0-beta.1`

### Requirement: Removing `prerelease` releases the stable version

A package whose current version is a prerelease and that omits `prerelease` SHALL be pending with no fragment.
`verctl version` SHALL write the base, bumped further only when a pending fragment needs it.

#### Scenario: The stable release with nothing new

- **WHEN** `Cargo.toml` holds `version = "0.8.0-rc.2"`, the package omits `prerelease`, and no fragment is pending
- **THEN** `verctl version` writes `version = "0.8.0"`

#### Scenario: A major that the base does not hold

- **WHEN** `Cargo.toml` holds `version = "1.3.0-rc.2"`, the package omits `prerelease`, and a fragment holds `verctl: major`
- **THEN** `verctl version` writes `version = "2.0.0"`

### Requirement: The stable section lists every candidate's changes

The CHANGELOG section of a stable version that follows candidates SHALL be rendered from the pending fragments and from each fragment a candidate since the last stable release consumed for that package.
`verctl version` SHALL read those fragments from the first-parent history.
The candidate sections SHALL stay in the CHANGELOG.

#### Scenario: Two candidates, then stable

- **WHEN** Version PR `0.8.0-rc.1` consumed `.changeset/new-config.md`, Version PR `0.8.0-rc.2` consumed `.changeset/fix-tag.md`, and the package then omits `prerelease`
- **THEN** `verctl version` prepends `## verctl 0.8.0` with the entries of both fragments
- **AND** `## verctl 0.8.0-rc.2` and `## verctl 0.8.0-rc.1` stay below it

#### Scenario: History too short to read

- **WHEN** the checkout is shallow and does not hold the Version PR commit of `0.8.0-rc.1`
- **THEN** `verctl version` exits non-zero, naming the commit it could not read
- **AND** no manifest or CHANGELOG is changed

### Requirement: A candidate's GitHub Release is a prerelease

`verctl release` SHALL create the Release of a version with a prerelease with `prerelease: true` and `make_latest: false`.
It SHALL create the Release of a stable version with GitHub's defaults.

#### Scenario: A candidate Release

- **WHEN** `verctl release` publishes `verctl` `0.8.0-rc.1`
- **THEN** it creates Release `v0.8.0-rc.1` with `prerelease: true` and `make_latest: false`

#### Scenario: The stable Release

- **WHEN** `verctl release` publishes `verctl` `0.8.0`
- **THEN** it creates Release `v0.8.0` with `prerelease: false` and sends no `make_latest`

### Requirement: A candidate keeps the npm `latest` tag where it was

The bun publisher SHALL add `--tag <prerelease tag>` to the upload of a version with a prerelease.
The cargo publisher and a replaced upload command SHALL upload a candidate with their usual argv.

#### Scenario: A bun candidate

- **WHEN** package `web` at `package.json` publishes `1.3.0-rc.1` with the bun publisher
- **THEN** `verctl release --dry-run` lists the upload `bun publish --tolerate-republish --tag rc`

#### Scenario: A cargo candidate

- **WHEN** crate `demo` publishes `0.8.0-rc.1` with the cargo publisher
- **THEN** `verctl release --dry-run` lists the upload `cargo publish` with no tag

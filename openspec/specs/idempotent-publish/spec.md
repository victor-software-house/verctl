# idempotent-publish Specification

## Purpose

When a release run publishes: only at a version commit, only the packages released there, and nothing once that release is complete, so a rerun is safe.

## Requirements

### Requirement: verctl publishes only at a version commit

`verctl publish` SHALL run only when HEAD is the version commit of at least one
package whose release is incomplete.

The version commit is the newest commit on the default branch's first-parent
history where that package's version became its version at HEAD.
On any other commit with an incomplete release it SHALL exit non-zero before
any side effect and name the version commit.

#### Scenario: HEAD has moved past the release

- **WHEN** commit `A` set `demo` to `0.4.6`, commit `B` followed without touching `Cargo.toml`, crates.io lacks `0.4.6`, and `verctl publish` runs at `B`
- **THEN** it exits non-zero before any publish and names commit `A`

### Requirement: A publish acts only on the packages released at HEAD

`verctl publish` SHALL publish, tag, and release only the packages whose
version commit is HEAD. It SHALL leave every other incomplete package
untouched and name it in the report.

#### Scenario: Another package is pending at an older commit

- **WHEN** commit `A` set `core` to `0.3.1`, later commit `C` set `app` to `0.2.1`, neither is published, and `verctl publish` runs at `C`
- **THEN** it publishes, tags, and releases only `app@0.2.1`, creates no tag for `core`, and its report names `core@0.3.1` and commit `A`

#### Scenario: The plan finds the commit

- **WHEN** the same history, and `verctl publish --plan` runs at `B`
- **THEN** it writes `commits=["<A>"]` and `pending=true` to `$GITHUB_OUTPUT`

#### Scenario: Every incomplete commit, oldest first

- **WHEN** commit `A` set `core` to `0.3.1` and later commit `C` set `app` to `0.2.1`, and neither release is complete
- **THEN** `verctl publish --plan` writes `commits=["<A>","<C>"]`

### Requirement: A complete release is nothing to release

With assets on, the declared assets count toward completeness; with
`--skip-assets` (`VERCTL_SKIP_ASSETS=true`) they do not.
A package's release is complete when its registry entry, tag, Release, and
counted assets exist for its version at HEAD.
When every package's release is complete, `verctl publish` and
`verctl publish --plan` SHALL report `nothing to release`, run no publisher,
create nothing, and exit zero.
The JSON report SHALL carry `"outcome": "nothing-to-release"`.

#### Scenario: A push after a finished release

- **WHEN** `demo@0.4.6` is on crates.io, tag `v0.4.6` and its Release exist with both declared assets, and `verctl publish --format json` runs
- **THEN** it exits zero with `"outcome": "nothing-to-release"`

#### Scenario: Assets turned off

- **WHEN** `.ctl/ver.yaml` declares assets for `demo`, none was ever uploaded, `demo@0.4.6` is on crates.io with its tag and Release, and `VERCTL_SKIP_ASSETS=true`
- **THEN** `verctl publish --plan` reports `nothing to release`

#### Scenario: The tag sits on a later commit at the same version

- **WHEN** commit `A` set `demo` to `0.4.6`, a publish rerun at the later commit `B` (no version change) created tag `v0.4.6` there, and `demo@0.4.6` is on crates.io with its Release
- **THEN** `verctl publish --plan` treats `B` as the release commit and reports `nothing to release`

#### Scenario: The tag sits on a commit at another version

- **WHEN** tag `v0.4.6` names a commit where `demo` is `0.4.5`
- **THEN** `verctl publish --plan` exits non-zero and names the tag, that commit, and the version commit

#### Scenario: Only an asset is missing

- **WHEN** the release of `demo@0.4.6` lacks only its `darwin-arm64` asset
- **THEN** `verctl publish --plan` reports the version commit as pending, `verctl assets` plans only `darwin-arm64`, and `verctl publish` at that commit reports the publish, tag, and Release as `already`

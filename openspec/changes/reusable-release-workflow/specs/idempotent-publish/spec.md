## ADDED Requirements

### Requirement: The registry decides whether a version is published

Before it runs a package's publisher, `verctl publish` SHALL run the recipe's
`query` and treat the package as published when the query exits zero and
prints the version. It SHALL then not run the publisher, and SHALL report the
package as `already`. After running a publisher it SHALL run the query again,
retrying with a bounded backoff, and fail unless the registry lists the
version within that bound. A recipe whose `query` is
`[]` SHALL keep the stderr match for "already published".

#### Scenario: A rerun after the tag push failed

- **WHEN** crates.io lists `demo` `0.4.6` and tag `v0.4.6` is missing
- **THEN** `verctl publish` at the version commit does not run `cargo publish`, creates `v0.4.6`, and reports `demo` as `already`

#### Scenario: A workspace member is not the registry

- **WHEN** crate `demo` is a member of the workspace at `0.4.6` and the registry index lacks `0.4.6`
- **THEN** the stock cargo `query` runs outside the workspace, does not print `0.4.6`, and `verctl publish` runs `cargo publish`

#### Scenario: A registry declared in the repository's Cargo config

- **WHEN** `.cargo/config.toml` at the repository root declares registry `internal`, and package `demo` sets `registry: internal`
- **THEN** the cargo `query` runs outside the workspace under a copy of that file with `--registry internal`, and reads `internal`'s index

#### Scenario: A package-level Cargo config and an environment override

- **WHEN** `crates/demo/.cargo/config.toml` declares registry `internal`, the root config declares it with another index, and CI sets `CARGO_REGISTRIES_INTERNAL_INDEX`
- **THEN** the cargo `query` for `demo` reads the index from `CARGO_REGISTRIES_INTERNAL_INDEX`, and without that variable the one from `crates/demo/.cargo/config.toml`

#### Scenario: A scoped npm registry

- **WHEN** `.npmrc` in a bun package's directory maps its scope to a private registry
- **THEN** the bun `query` runs in that directory and asks the private registry, not the public one

#### Scenario: A publisher that exits zero without publishing

- **WHEN** the publisher exits zero and `query` still does not print `0.4.6` when the backoff ends
- **THEN** `verctl publish` exits non-zero, names `demo@0.4.6`, and creates no tag

#### Scenario: Registry metadata lags the publish

- **WHEN** the publisher exits zero, the first `query` after it does not print `0.4.6`, and a retry within the bound does
- **THEN** `verctl publish` reports `demo@0.4.6` as published and goes on to the tag

### Requirement: verctl publishes only at a version commit

`verctl publish` SHALL run only when HEAD is the version commit of at least one
package whose release is incomplete: the newest commit on the default
branch's first-parent history where that package's version became its version
at HEAD. On any other commit with an incomplete release it SHALL exit non-zero
before any side effect and name the version commit.

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
When every package's registry entry, tag, Release, and counted assets exist
for its version at HEAD, `verctl publish` and `verctl publish --plan` SHALL
report `nothing to release`, run no publisher, create nothing, and exit zero.
The JSON report SHALL carry `"outcome": "nothing-to-release"`.

#### Scenario: A push after a finished release

- **WHEN** `demo@0.4.6` is on crates.io, tag `v0.4.6` and its Release exist with both declared assets, and `verctl publish --format json` runs
- **THEN** it exits zero with `"outcome": "nothing-to-release"`

#### Scenario: Assets turned off

- **WHEN** `.ctl/ver.yaml` declares assets for `demo`, none was ever uploaded, `demo@0.4.6` is on crates.io with its tag and Release, and `VERCTL_SKIP_ASSETS=true`
- **THEN** `verctl publish --plan` reports `nothing to release`

#### Scenario: Only an asset is missing

- **WHEN** the release of `demo@0.4.6` lacks only its `darwin-arm64` asset
- **THEN** `verctl publish --plan` reports the version commit as pending, `verctl assets` plans only `darwin-arm64`, and `verctl publish` at that commit reports the publish, tag, and Release as `already`

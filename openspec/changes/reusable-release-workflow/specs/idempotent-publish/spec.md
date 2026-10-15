## ADDED Requirements

### Requirement: The registry decides whether a version is published

For a package with a registry check, `verctl publish` SHALL ask the registry
before the publisher runs, and report `already` without running it when the
registry has the version. After the publisher runs it SHALL ask again with a
bounded backoff, and fail unless the registry lists the version. A package
with no registry check SHALL succeed on a zero exit or on stderr saying the
version already exists.

#### Scenario: A rerun after the tag push failed

- **WHEN** crates.io lists `demo` `0.4.6` and tag `v0.4.6` is missing
- **THEN** `verctl publish` at the version commit does not run `cargo publish`, creates `v0.4.6`, and reports `demo` as `already`

### Requirement: How verctl asks the registry

A cargo package whose publish target is crates.io and that declares no
`query` SHALL be checked by reading its crates.io sparse index file, at the
path built from the lowercased name, for a `vers` equal to the version. Any
other package with a `query` SHALL be checked by running it in the package's
directory. A package with `query: []`, or a cargo package with another
target and no `query`, SHALL have no registry check.

#### Scenario: A workspace member is not the registry

- **WHEN** crate `demo` is a member of the workspace at `0.4.6` and the crates.io index file for `demo` has no `0.4.6` line
- **THEN** `verctl publish` runs `cargo publish`

#### Scenario: A crate name with capitals

- **WHEN** crate `Demo_Crate` is at `0.4.6` and crates.io lists `0.4.6`
- **THEN** verctl reads `https://index.crates.io/de/mo/demo_crate`, finds `0.4.6`, and reports `already`

#### Scenario: A manifest that publishes elsewhere

- **WHEN** crate `demo` sets no `registry` in `.ctl/ver.yaml` but its `Cargo.toml` has `publish = ["internal"]`, and it declares no `query`
- **THEN** verctl reads no crates.io index, runs `cargo publish`, and succeeds on its zero exit

#### Scenario: A runner whose Cargo home names another default

- **WHEN** crate `demo` sets no `registry`, its manifest has no `publish` list, and `$CARGO_HOME/config.toml` sets `registry.default = "internal"`
- **THEN** verctl treats the target as not crates.io, reads no crates.io index, and runs `cargo publish`

#### Scenario: A package-level config the publisher never reads

- **WHEN** crate `demo` lives in `crates/demo`, `crates/demo/.cargo/config.toml` sets `registry.default = "internal"`, and the stock publisher runs from the repository root with `--manifest-path`
- **THEN** verctl ignores that file, as `cargo publish` does, and treats the target as crates.io

#### Scenario: A crate on another registry

- **WHEN** crate `demo` sets `registry: internal` and declares no `query`
- **THEN** verctl reads no index, runs `cargo publish`, succeeds on its zero exit, and treats an "already exists" error as `already`

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

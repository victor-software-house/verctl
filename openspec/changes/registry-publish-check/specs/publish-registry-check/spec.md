## ADDED Requirements

### Requirement: The registry decides whether a version is published

For a package with a registry check, `verctl publish` SHALL query the registry
before the publisher runs.
It SHALL report `already` without running the publisher when the registry has
the version.
After the publisher runs it SHALL query again with a bounded backoff,
and fail unless the registry lists the version.

A package with no registry check SHALL succeed on a zero exit or on stderr
that shows the version already exists.

#### Scenario: A rerun after the tag push failed

- **WHEN** crates.io lists `demo` `0.4.6` and tag `v0.4.6` is missing
- **THEN** `verctl publish` at the version commit does not run `cargo publish`, creates `v0.4.6`, and reports `demo` as `already`

#### Scenario: A publisher that exits zero without publishing

- **WHEN** the publisher exits zero and `query` still does not print `0.4.6` when the backoff ends
- **THEN** `verctl publish` exits non-zero, names `demo@0.4.6`, and creates no tag

#### Scenario: Registry metadata lags the publish

- **WHEN** the publisher exits zero, the first `query` after it does not print `0.4.6`, and a retry within the bound does
- **THEN** `verctl publish` reports `demo@0.4.6` as published and goes on to the tag

### Requirement: How verctl asks the registry

A cargo package SHALL be checked by reading its crates.io sparse index file
when all of these hold:

- it uses the stock publisher
- its publish target is crates.io
- it declares no `query`

The path is built from the lowercased name.
The check SHALL look in that file for a `vers` equal to the version.
Any other package with a `query` SHALL be checked by running it in the
package's directory.

#### Scenario: A workspace member is not the registry

- **WHEN** crate `demo` is a member of the workspace at `0.4.6` and the crates.io index file for `demo` has no `0.4.6` line
- **THEN** `verctl publish` runs `cargo publish`

#### Scenario: A crate name with capitals

- **WHEN** crate `Demo_Crate` is at `0.4.6` and crates.io lists `0.4.6`
- **THEN** verctl reads `https://index.crates.io/de/mo/demo_crate`, finds `0.4.6`, and reports `already`

#### Scenario: A package-level config the publisher never reads

- **WHEN** crate `demo` lives in `crates/demo`, `crates/demo/.cargo/config.toml` sets `registry.default = "internal"`, and the stock publisher runs from the repository root with `--manifest-path`
- **THEN** verctl ignores that file, as `cargo publish` does, and treats the target as crates.io

#### Scenario: A scoped npm registry

- **WHEN** `.npmrc` in a bun package's directory maps its scope to a private registry
- **THEN** the bun `query` runs in that directory and queries only the private registry

### Requirement: Packages without a registry check

A package SHALL have no registry check in any of these cases:

- it has `query: []`
- its upload command is overridden, whatever its publisher, and it declares no
  `query`
- it is a cargo package with no `query` whose target is another registry or
  unknown

#### Scenario: A manifest that publishes elsewhere

- **WHEN** crate `demo` sets no `registry` in `.ctl/ver.yaml` but its `Cargo.toml` has `publish = ["internal"]`, and it declares no `query`
- **THEN** verctl reads no crates.io index, runs `cargo publish`, and succeeds on its zero exit

#### Scenario: A runner whose Cargo home names another default

- **WHEN** crate `demo` sets no `registry`, its manifest has no `publish` list, and `$CARGO_HOME/config.toml` sets `registry.default = "internal"`
- **THEN** verctl treats the target as not crates.io, reads no crates.io index, and runs `cargo publish`

#### Scenario: An overridden cargo publisher

- **WHEN** crate `demo` replaces the stock publisher with its own argv and declares no `query`
- **THEN** verctl treats the target as unknown, reads no crates.io index, and runs the publisher

#### Scenario: An overridden bun publisher

- **WHEN** bun package `web` replaces the stock publisher with `npm publish --registry` pointed at the [example registry][example-registry] and declares no `query`
- **THEN** verctl runs no `bun pm view` query and runs the publisher

#### Scenario: A crate on another registry

- **WHEN** crate `demo` sets `registry: internal` and declares no `query`
- **THEN** verctl reads no index, runs `cargo publish`, succeeds on its zero exit, and treats an "already exists" error as `already`

[example-registry]: https://registry.example.com

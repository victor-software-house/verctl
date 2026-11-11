## ADDED Requirements

### Requirement: A manifest that opts out releases on GitHub only

A package whose publisher is inferred from its manifest file name SHALL release as `publisher: none` does when its manifest opts out of publishing.
A `Cargo.toml` opts out with `package.publish` set to `false` or `[]`, read through `publish.workspace = true` from the workspace root.
A `package.json` opts out with `"private": true`.
A virtual workspace manifest SHALL NOT count: its members decide one by one.

#### Scenario: A private binary with nothing written

- **WHEN** `.ctl/ver.yaml` declares `demo` at `Cargo.toml` with no `publisher`, and the manifest sets `publish = false`
- **THEN** `verctl release --dry-run` lists `demo` via `release only` and would create `v0.0.1`

#### Scenario: The setting is inherited from the workspace

- **WHEN** `crates/demo/Cargo.toml` sets `publish.workspace = true` and the root `Cargo.toml` sets `workspace.package.publish = false`
- **THEN** `verctl release --dry-run` lists `demo` via `release only`

#### Scenario: A private npm package

- **WHEN** package `web` at `package.json` declares no `publisher` and the manifest sets `"private": true`
- **THEN** `verctl release --dry-run` lists `web` via `release only`

### Requirement: A written publisher wins over the manifest

A package that names its `publisher`, declares a `publish` list, or uses a publisher whose `publishers` entry holds an `argv` SHALL be published as written, whatever its manifest says.

#### Scenario: A named publisher wins

- **WHEN** crate `demo` declares `publisher: cargo` and its manifest sets `publish = false`
- **THEN** `verctl release --dry-run` lists `demo` via `cargo`

### Requirement: A manifest that opts out takes no publish settings

A package whose manifest opts out and that also sets `registry` or a `publish` mapping SHALL fail the plan.
The error SHALL name the package, the manifest key, and the settings to remove.

#### Scenario: Options beside a manifest that opts out

- **WHEN** crate `demo` declares `publish: {verify: false}` and its manifest sets `publish = false`
- **THEN** `verctl status` exits non-zero and its error names `demo`, `publish = false`, and the `publish` setting to remove

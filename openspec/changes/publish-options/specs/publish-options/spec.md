## ADDED Requirements

### Requirement: A package sets options for its publisher

A package's `publish` SHALL take a mapping of options for the publisher the package resolves to.
A package with such a mapping SHALL keep that publisher's upload command, its registry check, and its tag order.
A list under `publish` SHALL replace the whole upload command.
A `publish` that is neither a mapping nor a list SHALL fail the load and say it takes options or a command.

#### Scenario: Options keep the registry check

- **WHEN** `.ctl/ver.yaml` declares `demo` at `Cargo.toml` with `publish: {verify: false}`, and crates.io lists `demo` `0.4.6`
- **THEN** `verctl release` at the version commit reads the crates.io index, does not run `cargo publish`, and reports `demo` as `already`

#### Scenario: A list still replaces the command

- **WHEN** `demo` declares `publish: [cargo, publish, --no-verify]`
- **THEN** `verctl release` runs that command, reads no crates.io index, and creates the tag after the upload

#### Scenario: A value that is neither

- **WHEN** `demo` declares `publish: fast`
- **THEN** every command that loads `.ctl/ver.yaml` exits non-zero and its error says `publish` takes a mapping of options or a list that is the whole upload command

### Requirement: Options are checked against the publisher at load

The load SHALL fail when a package's `publish` mapping holds a key its publisher has no option for.
The error SHALL name the package, the publisher, the key, and the options that publisher has.
The load SHALL fail the same way when the publisher has no options at all:
a publisher declared under `publishers`, the built-in bun publisher, and a built-in publisher whose `publishers` entry replaces its `argv`.
An option whose value has the wrong type SHALL fail the load and name the type it takes.

#### Scenario: A cargo option on a bun package

- **WHEN** package `web` at `package.json` declares `publish: {verify: false}`
- **THEN** `verctl status` exits non-zero and its error names `web`, `bun`, and `verify`, and says bun has no publish options

#### Scenario: A misspelt option

- **WHEN** crate `demo` at `Cargo.toml` declares `publish: {verfy: false}`
- **THEN** `verctl status` exits non-zero and its error names `demo`, `cargo`, `verfy`, and `verify`

#### Scenario: A value of the wrong type

- **WHEN** crate `demo` declares `publish: {verify: "no"}`
- **THEN** `verctl status` exits non-zero and its error says `verify` is `true` or `false`

#### Scenario: A replaced cargo command has no options

- **WHEN** `.ctl/ver.yaml` declares `publishers.cargo.argv: [cargo, publish]` and crate `demo` declares `publish: {verify: false}`
- **THEN** `verctl status` exits non-zero and its error says `publishers.cargo` replaces the upload command, which takes no options

### Requirement: verify controls cargo's build of the packed crate

The built-in cargo publisher SHALL take the option `verify`, `true` when omitted.
With `verify: false` its upload command SHALL end with `--no-verify`.
With `verify: true`, or with no `publish` mapping, the upload command SHALL be the one verctl runs today.

#### Scenario: A crate CI already packed

- **WHEN** crate `demo` at `Cargo.toml` declares `publish: {verify: false}` and crates.io does not list its version
- **THEN** `verctl release` runs `cargo publish --locked --manifest-path Cargo.toml --no-verify`

#### Scenario: Omitted

- **WHEN** crate `demo` declares no `publish`
- **THEN** `verctl release` runs `cargo publish --locked --manifest-path Cargo.toml`

### Requirement: The plan shows the options in effect

`verctl status` and `verctl release --dry-run` SHALL list a package whose cargo upload skips the verify build as via `cargo, no verify`.
A package with every option at its default SHALL be listed as before.

#### Scenario: A dry run of a crate that skips verify

- **WHEN** crate `demo` declares `publish: {verify: false}` and has an unreleased version
- **THEN** `verctl release --dry-run` lists `demo` via `cargo, no verify`

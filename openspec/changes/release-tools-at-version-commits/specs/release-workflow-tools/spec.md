## ADDED Requirements

### Requirement: The plan lists each version commit's publisher tools

The plan job of `release.yml` SHALL output, for each pending version commit, the tools its packages' publishers run.
The built-in cargo publisher SHALL add `rust`, the built-in bun publisher SHALL add `bun`, and a custom publisher SHALL add `all`.

#### Scenario: A cargo and a custom publisher

- **WHEN** one version commit holds a cargo package and a package whose publisher is a command from the config
- **THEN** the plan job's `release-tools` output for that commit is `rust all`

### Requirement: The publish job installs only the listed tools

The publish job SHALL install only the listed tools with `mise install --locked`, and SHALL run the release with `MISE_EXEC_AUTO_INSTALL=false`.
A list that holds `all` SHALL install the whole config.
When the version commit's rust entry sets `mr_boxington`, the job SHALL also install `mr-boxington`.
No job SHALL set `MISE_LOCKED_REQUIRE_CHECKSUM`.

#### Scenario: An old lock lacks a checksum for an unrelated tool

- **WHEN** a version commit's `mise.lock` has no checksum for taplo, and its packages use the built-in cargo publisher
- **THEN** the publish job runs `mise install --locked rust`, does not install taplo, and runs `cargo publish` for each package

#### Scenario: A custom publisher

- **WHEN** a package's publisher is a command from the config, and `release-tools` is empty
- **THEN** the publish job installs the whole config with `mise install --locked`

### Requirement: The docs job installs no tool at the version commit

The docs job SHALL NOT run `mise install` at the version commit, and SHALL install only the plan's verctl, from HEAD's lock.

#### Scenario: docs.rs at an old version commit

- **WHEN** the docs job runs for a version commit whose lock lacks a checksum for any tool
- **THEN** the job installs the plan's verctl from HEAD's lock and waits for docs.rs without installing that commit's tools

### Requirement: The caller can name the tools

`release.yml` SHALL take a `release-tools` input, a space-separated list of mise tools, empty by default.
A non-empty value SHALL replace the plan's list for every version commit in the run, and `all` SHALL install the whole config.

#### Scenario: A build script needs another tool

- **WHEN** the caller passes `release-tools: rust protoc`
- **THEN** the publish job runs `mise install --locked rust protoc`

#### Scenario: Today's behaviour

- **WHEN** the caller passes `release-tools: all`
- **THEN** the publish job runs `mise install --locked` with no tool named

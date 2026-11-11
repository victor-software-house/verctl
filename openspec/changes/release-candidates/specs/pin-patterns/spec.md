## ADDED Requirements

### Requirement: A pin reads and writes a prerelease version

A bounded pattern's `{version}` SHALL match a whole semver version, its prerelease and build included.
`verctl version` SHALL rewrite that whole version to the new one.

#### Scenario: From a candidate to the next

- **WHEN** `skills/demo/SKILL.md` holds `version: 0.8.0-rc.1`, its pattern is `version: {version}` with `whole_line: true`, and `verctl version` moves `demo` to `0.8.0-rc.2`
- **THEN** `skills/demo/SKILL.md` holds `version: 0.8.0-rc.2`

#### Scenario: From a candidate to stable

- **WHEN** `README.md` holds `verctl@0.8.0-rc.2 in CI`, its pattern is `verctl@{version} in CI`, and `verctl version` moves `verctl` to `0.8.0`
- **THEN** `README.md` holds `verctl@0.8.0 in CI`

### Requirement: A tool pin moves a prerelease include

A `tool` pin SHALL move each include of the tool's own repository whose `?ref=v` value is a whole semver version, prerelease included.
A `?ref=v` value that starts with a digit and is not a whole version SHALL fail the pin and leave its file unchanged.

When a `?ref=v` value does not start with a digit, it SHALL stay unchanged and SHALL NOT fail the pin.

#### Scenario: A candidate ref

- **WHEN** `mise.toml` pins `github:victor-software-house/verctl` and includes `verctl.git//tasks/ver?ref=v0.8.0-rc.1`, and `verctl version` moves `verctl` to `0.8.0`
- **THEN** `mise.toml` includes `verctl.git//tasks/ver?ref=v0.8.0`

#### Scenario: A ref that is not a version

- **WHEN** `mise.toml` includes `verctl.git//tasks/ver?ref=v1.2.3_local`, and `verctl version` bumps `verctl`
- **THEN** it exits non-zero, naming `mise.toml` and `v1.2.3_local`
- **AND** `mise.toml` is unchanged

## REMOVED Requirements

### Requirement: A prerelease in a pinned file stops the release

**Reason**: Release candidates write prerelease versions, and the next Version PR must move a pin that names one.
The bounded sides that made the refusal safe still keep a pattern from matching part of a version.

**Migration**: None. A file that held a prerelease used to stop the release, and now it is rewritten.

### Requirement: A tool pin refuses a prerelease include

**Reason**: Replaced by "A tool pin moves a prerelease include".
That requirement keeps the refusal for a ref that starts with a digit and is not a whole version.

**Migration**: None.

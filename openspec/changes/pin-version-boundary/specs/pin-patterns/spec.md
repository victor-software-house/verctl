## ADDED Requirements

### Requirement: A pattern bounds the version on both sides

Loading `.ctl/ver.yaml` SHALL fail on a pattern whose `match` has no boundary on either side of `{version}`.
A side's literal is the text between `{version}` and the pattern's edge, on that side.
A side SHALL be bounded when its literal holds at least one character that is not an ASCII digit, `.`, `-`, or `+`.

The message SHALL name the pattern id and the side, and SHALL say what to change in `match` on that side.

#### Scenario: A match that ends at the placeholder

- **WHEN** `.ctl/ver.yaml` declares `patterns.skill.match: "version: {version}"` without `whole_line`, and `verctl check` runs
- **THEN** it exits non-zero
- **AND** the message names `skill` and `whole_line: true`

#### Scenario: A match that ends in a dot

- **WHEN** `.ctl/ver.yaml` declares `patterns.mention.match: "verctl@{version}."`, and `verctl check` runs
- **THEN** it exits non-zero and names `mention`

#### Scenario: A lone dash after the placeholder

- **WHEN** `.ctl/ver.yaml` declares `patterns.asset.match: "v{version}-"`, and `verctl check` runs
- **THEN** it exits non-zero and names `asset` and the `-`

#### Scenario: A file name on both sides

- **WHEN** `.ctl/ver.yaml` declares `patterns.tarball.match: "name-{version}.tar.gz"`, and `verctl check` runs
- **THEN** the pattern loads

#### Scenario: A file name after the version

- **WHEN** `patterns.asset.match` is `download/v{version}-linux` and the pinned file holds `download/v1.2.3-linux-x64.tar.gz`
- **THEN** the pattern loads and `version` rewrites `1.2.3`

### Requirement: A digit next to the placeholder leaves a side open

A side SHALL be open, whatever the rest of its literal holds, when its character next to `{version}` is an ASCII digit, or a `.` followed outward by an ASCII digit.

#### Scenario: A digit next to the placeholder

- **WHEN** `.ctl/ver.yaml` declares `patterns.asset.match: "v{version}.5x"`, and `verctl check` runs
- **THEN** it exits non-zero and names `asset` and `.5x`

#### Scenario: A digit before the placeholder

- **WHEN** `.ctl/ver.yaml` declares `patterns.asset.match: "x1{version}-linux"`, and `verctl check` runs
- **THEN** it exits non-zero and names `asset` and `x1`

### Requirement: A line bounds a line-owning pattern

With `whole_line: true`, the line start or end SHALL bound a side, unless the literal on that side is made only of digits, `.`, `-`, and `+`.

#### Scenario: A line-owning pattern with a dash after the version

- **WHEN** `.ctl/ver.yaml` declares `patterns.skill.match: "version: {version}-"` with `whole_line: true`, and `verctl check` runs
- **THEN** it exits non-zero and names `skill`

#### Scenario: A line-owning pattern

- **WHEN** `.ctl/ver.yaml` declares `patterns.skill.match: "version: {version}"` with `whole_line: true`, and `verctl check` runs
- **THEN** the pattern loads

### Requirement: A prerelease in a pinned file stops the release

`verctl version` SHALL fail, and SHALL leave the file unchanged, when a bounded pattern's file holds a prerelease where the version goes.

#### Scenario: A prerelease frontmatter key

- **WHEN** `skills/demo/SKILL.md` holds `version: 0.1.1-rc1`, its pattern is `version: {version}` with `whole_line: true`, and a fragment bumps `demo`
- **THEN** `verctl version` exits non-zero, naming the file and the pattern
- **AND** `skills/demo/SKILL.md` still holds `version: 0.1.1-rc1`

### Requirement: A tool pin refuses a prerelease include

A `tool` pin moves each include of the tool's own repository.
Such an include may have a `?ref=v` value that starts with a digit and is not a whole version.
The pin SHALL then fail and SHALL leave its file unchanged.
A `?ref=v` value that does not start with a digit SHALL stay unchanged and SHALL NOT fail the pin.

#### Scenario: A prerelease ref

- **WHEN** `mise.toml` pins `github:victor-software-house/verctl` and includes `verctl.git//tasks/ver?ref=v1.2.3-rc1`, and `verctl version` bumps `verctl`
- **THEN** it exits non-zero, naming `mise.toml` and `v1.2.3-rc1`
- **AND** `mise.toml` is unchanged

## ADDED Requirements

### Requirement: A pattern bounds the version on both sides

Loading `.ctl/ver.yaml` SHALL fail on a pattern whose `match` has no boundary on either side of `{version}`.
A side SHALL have a boundary when the character next to `{version}` on that side is neither an ASCII digit nor `.`.
The message SHALL name the pattern id and the side, and SHALL say to add the neighbouring text to `match` or to set `whole_line: true`.

#### Scenario: A match that ends at the placeholder

- **WHEN** `.ctl/ver.yaml` declares `patterns.skill.match: "version: {version}"` without `whole_line`, and `verctl check` runs
- **THEN** it exits non-zero
- **AND** the message names `skill` and `whole_line: true`

#### Scenario: A match that ends in a dot

- **WHEN** `.ctl/ver.yaml` declares `patterns.mention.match: "verctl@{version}."`, and `verctl check` runs
- **THEN** it exits non-zero and names `mention`

#### Scenario: A file name after the version

- **WHEN** `patterns.asset.match` is `download/v{version}-linux` and the pinned file holds `download/v1.2.3-linux-x64.tar.gz`
- **THEN** the pattern loads and `version` rewrites `1.2.3`

### Requirement: A line bounds a line-owning pattern

With `whole_line: true`, the start of the line SHALL bound a `match` that starts with `{version}`, and the end of the line SHALL bound one that ends with it.

#### Scenario: A line-owning pattern

- **WHEN** `.ctl/ver.yaml` declares `patterns.skill.match: "version: {version}"` with `whole_line: true`, and `verctl check` runs
- **THEN** the pattern loads

### Requirement: A prerelease in a pinned file stops the release

`verctl version` SHALL fail, and SHALL leave the file unchanged, when a bounded pattern's file holds a prerelease where the version goes.

#### Scenario: A prerelease frontmatter key

- **WHEN** `skills/demo/SKILL.md` holds `version: 0.1.1-rc1`, its pattern is `version: {version}` with `whole_line: true`, and a fragment bumps `demo`
- **THEN** `verctl version` exits non-zero, naming the file and the pattern
- **AND** `skills/demo/SKILL.md` still holds `version: 0.1.1-rc1`

## ADDED Requirements

### Requirement: The version step is the version command

`verctl version` SHALL apply fragment bumps and take the flags and
variables `prepare` took. Its report SHALL list follow-up commands under
`after`.

#### Scenario: A dry run in JSON

- **WHEN** `.changeset/fix.md` names `demo: patch` and `verctl version --dry-run --format json` runs
- **THEN** the report has an `after` array and no `next` field

### Requirement: The pin rewrite is the render command

`verctl render` SHALL rewrite each `pins` entry to the versions on HEAD and
take the flags `pin` took.

#### Scenario: A dry run

- **WHEN** `.ctl/ver.yaml` declares a pin on `mise.release.toml` and `verctl render --dry-run` runs
- **THEN** it lists `mise.release.toml` and writes nothing

### Requirement: A removed verb names its replacement

`verctl prepare` and `verctl pin` SHALL exit 2 without reading the config,
and their error SHALL name the command that replaced them. Neither SHALL
appear in `--help` or in the usage spec.

#### Scenario: The old version verb

- **WHEN** a user runs `verctl prepare --pr`
- **THEN** it exits 2 and stderr says `prepare is now version: run verctl version`

#### Scenario: The old pin verb under JSON

- **WHEN** a user runs `verctl --format json pin --dry-run`
- **THEN** it exits 2 and the JSON error on stdout says `pin is now render: run verctl render`

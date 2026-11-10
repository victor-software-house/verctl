## ADDED Requirements

### Requirement: The version step is the version command

`verctl version` SHALL apply fragment bumps and take every flag and
variable the version step took in verctl 0.7. Its report SHALL list
follow-up commands under `after`.

#### Scenario: A dry run in JSON

- **WHEN** `.changeset/fix.md` names `demo: patch` and `verctl version --dry-run --format json` runs
- **THEN** the report has an `after` array and no `next` field

### Requirement: The render command writes pins and served files

`verctl render` SHALL rewrite each `pins` entry, then render every template
in `.ctl/templates/`, at the versions on HEAD. It SHALL take every flag the
pins rewrite took in verctl 0.7.

#### Scenario: A dry run

- **WHEN** `.ctl/ver.yaml` declares a `pins` entry for `mise.release.toml` and `verctl render --dry-run` runs
- **THEN** it lists `mise.release.toml` and writes nothing

#### Scenario: A served file

- **WHEN** `.ctl/templates/ver.jinja` renders `tasks/ver/ver` and `verctl render` runs
- **THEN** `tasks/ver/ver` holds the template's output at the versions on HEAD

### Requirement: A removed verb names its replacement

`verctl prepare` and `verctl pin` SHALL exit 2 without reading the config,
and their error SHALL name the command that replaced them. Neither SHALL
appear in `--help`. The usage spec SHALL mark both `hide`, which keeps
them out of mise help and completion.

#### Scenario: The old version verb

- **WHEN** a user runs `verctl prepare --pr`
- **THEN** it exits 2 and stderr says `prepare is now version: run verctl version`

#### Scenario: The old pin verb under JSON

- **WHEN** a user runs `verctl --format json pin --dry-run`
- **THEN** it exits 2 and the JSON error on stdout says `pin is now render: run verctl render`

#### Scenario: The usage spec

- **WHEN** mise runs `verctl --usage-spec=ver` to read the task grammar
- **THEN** the `cmd prepare` and `cmd pin` nodes carry `hide=#true`

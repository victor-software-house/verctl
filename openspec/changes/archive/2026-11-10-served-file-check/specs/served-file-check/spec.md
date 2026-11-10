## ADDED Requirements

### Requirement: check compares served files with their templates

`verctl check` SHALL render every template in `.ctl/templates/` at the
versions the manifests hold, and SHALL fail when a target's bytes differ from
the rendering. It SHALL do so whether or not `.changeset/` holds fragments,
and SHALL name every differing target with its template before it exits.

#### Scenario: A served file edited by hand

- **WHEN** `.ctl/templates/ver.jinja` renders `tasks/ver/ver`, someone edits `tasks/ver/ver` by hand, `.changeset/` is empty, and `verctl check` runs
- **THEN** it exits non-zero
- **AND** the message names `tasks/ver/ver` and `.ctl/templates/ver.jinja`

#### Scenario: Every served file matches

- **WHEN** each target equals its rendering and `verctl check` runs
- **THEN** it exits zero

### Requirement: check verifies each pins entry

`verctl check` SHALL fail when the pattern of a `pins` entry matches its file
a number of times other than its declared count.
`verctl check` SHALL also fail when the file of the entry names a version other
than the one the manifest holds.

#### Scenario: A pins entry left behind

- **WHEN** a `pins` entry on `tools.toml` matches `demo` at 0.7.3, `Cargo.toml` holds `demo` 0.7.4, and `verctl check` runs
- **THEN** it exits non-zero and names `tools.toml`

### Requirement: A served file has one owner

`verctl version`, `verctl check`, and `verctl render` SHALL fail before
writing anything when a template's target is also the file of a `pins`
entry. The message SHALL name the file, the template, and the `pins` entry.

#### Scenario: A template and a pins entry claim one file

- **WHEN** `.ctl/templates/tools.toml.jinja` renders `tools.toml`, a `pins` entry names `tools.toml`, a fragment bumps its package, and `verctl version` runs
- **THEN** it exits non-zero before writing anything
- **AND** the message names `tools.toml`, the template, and the `pins` entry

### Requirement: render fixes what check refuses

`verctl render` SHALL rewrite every `pins` entry and render every template
at the versions the manifests hold, and the message of a failed `check`
SHALL name `verctl render`.

#### Scenario: A hand edit reverted

- **WHEN** `tasks/ver/ver` was edited by hand, `verctl check` fails, and `verctl render` runs
- **THEN** `tasks/ver/ver` holds its template's output again and `verctl check` exits zero

#### Scenario: A template changed with no fragment pending

- **WHEN** `.ctl/templates/ver.jinja` is edited, `.changeset/` is empty, and `verctl render` runs
- **THEN** `tasks/ver/ver` holds the new output and `verctl check` exits zero

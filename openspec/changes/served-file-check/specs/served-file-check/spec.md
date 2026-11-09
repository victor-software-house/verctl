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

### Requirement: check verifies each pin

`verctl check` SHALL fail when a `pins` entry's pattern no longer matches at
its declared count, or when its file names a version other than the one the
manifest holds.

#### Scenario: A pin left behind

- **WHEN** a `pins` entry on `tools.toml` matches `demo` at 0.7.3, `Cargo.toml` holds `demo` 0.7.4, and `verctl check` runs
- **THEN** it exits non-zero and names `tools.toml`

### Requirement: A served file has one owner

Loading `.ctl/ver.yaml` SHALL fail when a template's target is also the file
of a `pins` entry. The message SHALL name the file, the template, and the
`pins` entry.

#### Scenario: A template and a pin claim one file

- **WHEN** `.ctl/templates/tools.toml.jinja` renders `tools.toml` and a `pins` entry names `tools.toml`
- **THEN** `verctl status` exits non-zero before writing anything
- **AND** the message names `tools.toml`, the template, and the `pins` entry

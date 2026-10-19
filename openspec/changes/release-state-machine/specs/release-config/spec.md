## ADDED Requirements

### Requirement: Every concern has one key

`.ctl/ver.yaml` SHALL declare each concern in exactly one place: the branch in
`branch`, registries in `registries` named once and listed by name from
`packages[].registry`, ecosystems in `ecosystems` listed by name from
`packages[].ecosystem`, steps in `steps` listed by name from `at` and the
blocks that use them, the tag shape in `tags`, the Version PR text in
`proposal`, binaries in `packages[].assets`, and served files in `templates`.
No key SHALL exist whose only effect is printed output.

#### Scenario: Defaults for a single crate

- **WHEN** `.ctl/ver.yaml` declares only `packages: [{name: demo, path: Cargo.toml}]`
- **THEN** `verctl status --format json` reports branch `main`, registry `crates-io` with auth `token`, tag shape `v{version}`, and proposal branch `verctl/release/main`

#### Scenario: An undeclared name

- **WHEN** `at.verify` lists `smoke` and `steps` declares no `smoke`
- **THEN** `verctl status` exits non-zero and names `smoke` and the declared steps

#### Scenario: A step nothing lists

- **WHEN** `steps` declares `announce` and no point or block lists it
- **THEN** `verctl status` exits non-zero and names `announce`

### Requirement: Unknown keys are refused at every level

verctl SHALL refuse an unknown key in every mapping of `.ctl/ver.yaml`,
including inside `packages`, `registries`, `ecosystems`, `steps`, `at`,
`proposal`, `templates`, `runners`, and `ci`, and SHALL place the error on its
line.

#### Scenario: A misspelled nested key

- **WHEN** a package declares `chnagelog: docs/CHANGELOG.md`
- **THEN** `verctl status` exits non-zero and its error names `chnagelog` on its line

### Requirement: A removed key names its replacement

verctl SHALL refuse every key this change removes and SHALL name what replaces
it.

#### Scenario: A leftover prepare section

- **WHEN** `.ctl/ver.yaml` declares `prepare: {after: [cargo, update, --workspace], stage: [Cargo.lock]}`
- **THEN** `verctl status` exits non-zero and its error says the cargo Lockfile block refreshes `Cargo.lock`, and that other commands become `version` steps with `writes`

#### Scenario: A leftover pin

- **WHEN** `.ctl/ver.yaml` declares `pins`
- **THEN** `verctl status` exits non-zero and its error names `templates` as the replacement

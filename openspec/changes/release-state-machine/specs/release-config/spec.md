## ADDED Requirements

### Requirement: Every concern has one key

`.ctl/ver.yaml` SHALL declare each concern in exactly one place: the default
branch in `branch`, registries in `registries` named once and listed by name
from `packages[].registry`, the tag shape in `tags`, the Version PR in
`proposal`, native binaries in `packages[].assets`, and served files in
`templates`. No key SHALL exist whose only effect is printed output.

#### Scenario: Defaults for a single crate

- **WHEN** `.ctl/ver.yaml` declares only `packages: [{name: demo, path: Cargo.toml}]`
- **THEN** `verctl status --format json` reports branch `main`, registry `crates-io` with auth `oidc`, tag shape `v{version}`, and proposal branch `version-packages`

#### Scenario: An undeclared registry

- **WHEN** a package names `registry: internal` and `registries` declares no `internal`
- **THEN** `verctl status` exits non-zero and names `internal` and the declared registries

### Requirement: Unknown keys are refused at every level

verctl SHALL refuse an unknown key in every mapping of `.ctl/ver.yaml`,
including inside `packages`, `registries`, `proposal`, `templates`, `runners`,
`ci`, and `ecosystem.command`, and SHALL place the error on its line.

#### Scenario: A misspelled nested key

- **WHEN** a package declares `chnagelog: docs/CHANGELOG.md`
- **THEN** `verctl status` exits non-zero and its error names `chnagelog` on its line

### Requirement: A removed key names its replacement

verctl SHALL refuse every key this change removes and SHALL name what replaces
it.

#### Scenario: A leftover prepare section

- **WHEN** `.ctl/ver.yaml` declares `prepare: {after: [cargo, update, --workspace], stage: [Cargo.lock]}`
- **THEN** `verctl status` exits non-zero and its error says the cargo ecosystem refreshes `Cargo.lock` and `prepare` is removed

#### Scenario: A leftover pin

- **WHEN** `.ctl/ver.yaml` declares `pins`
- **THEN** `verctl status` exits non-zero and its error names `templates` as the replacement

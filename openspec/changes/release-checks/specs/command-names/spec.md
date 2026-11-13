## REMOVED Requirements

### Requirement: A removed verb names the replacement verb

**Reason**: verctl keeps no hidden command or flag for an old name (operator, 2026-11-13), which reverses that part of the 0.8.0 renames (operator, 2026-11-09).
The old verbs and `release --plan` go, as `check --versions` does.

**Migration**: run `verctl version` for `prepare`, `verctl render` for `pin`, `verctl release` for `publish`, `verctl release asset <target>` for `assets`, and `verctl status --github-output <file>` for `release --plan`.

## ADDED Requirements

### Requirement: An old verb is an unknown command

`verctl prepare`, `pin`, `publish` and `assets` SHALL fail as unknown commands, and `verctl release --plan` SHALL fail as an unknown flag.
verctl SHALL keep no hidden command or flag for them.

#### Scenario: The old publish verb

- **WHEN** `verctl publish --dry-run` runs
- **THEN** it exits 2 with clap's unrecognized subcommand error, and reads no config

#### Scenario: The old plan flag

- **WHEN** `verctl release --plan` runs
- **THEN** it exits 2 with clap's unexpected argument error

#### Scenario: Help

- **WHEN** `verctl --help` runs
- **THEN** its output and the usage spec list none of `prepare`, `pin`, `publish` and `assets`

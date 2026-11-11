## ADDED Requirements

### Requirement: A fragment may mark a patch as breaking

verctl SHALL read `patch!` as a package's bump: a patch release that breaks the package's API on purpose.
The package SHALL be bumped by a patch.
verctl SHALL fail the parse on `minor!`, `major!`, and `none!`, with a message that says `!` goes only on patch.

#### Scenario: A breaking patch

- **WHEN** `.changeset/publish-options.md` sets `verctl: patch!`, and `Cargo.toml` holds `version = "0.8.0"`
- **THEN** `verctl version` writes `version = "0.8.1"`

#### Scenario: The mark on a minor

- **WHEN** `.changeset/break.md` sets `demo: minor!`, and `verctl check` runs
- **THEN** it exits non-zero, naming `.changeset/break.md` and saying `!` goes only on patch

#### Scenario: One package of two

- **WHEN** a fragment sets `demo-core: patch!` and `demo-cli: patch`
- **THEN** only `demo-core` carries the mark

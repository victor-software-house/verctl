## ADDED Requirements

### Requirement: `release bootstrap` is `release --new` then `release trust`

`verctl release bootstrap [CRATE]...` SHALL run the `release --new` code path, then the `release trust` code path for each crate that run created.
It SHALL take every option `verctl release` takes.

#### Scenario: A new crate

- **WHEN** crates.io lacks `shrub-process` and `verctl release bootstrap` runs with both tokens
- **THEN** it releases with `--new`, creates `shrub-process`, and adds its trusted publisher entry
- **AND** it adds no entry to a crate it did not create

### Requirement: `release bootstrap` creates only the crates it names

With crate names, only those crates SHALL be creatable, and another crate crates.io lacks SHALL fail the release proof as it does without `--new`.
Several crates MAY be named, and an unknown name SHALL fail before any request, listing the known crate names.

#### Scenario: Two named crates

- **WHEN** crates.io lacks `demo-core` and `demo-cli` and `verctl release bootstrap demo-core demo-cli` runs
- **THEN** one release creates both, and each gets its entry

#### Scenario: Another crate the registry lacks

- **WHEN** crates.io lacks `demo-core` and `demo-cli` and `verctl release bootstrap demo-cli` runs
- **THEN** the release proof fails naming `demo-core`, and nothing is uploaded

#### Scenario: An unknown name

- **WHEN** `verctl release bootstrap nosuch` runs
- **THEN** it exits non-zero before any request, names `nosuch`, and lists the crates the repository uploads

### Requirement: `release bootstrap` needs both tokens

A real run SHALL need `VERCTL_REGISTRY_TOKEN` and `VERCTL_OWNER_TOKEN`, both checked before any write, each missing one named, so no crate is created without its entry.
`--dry-run` SHALL need neither, SHALL need `GITHUB_TOKEN` for the reads a release makes, and SHALL print what both steps would do.

#### Scenario: No token

- **WHEN** `VERCTL_OWNER_TOKEN` is unset and `verctl release bootstrap` runs
- **THEN** it exits non-zero, names `VERCTL_OWNER_TOKEN`, and writes nothing

#### Scenario: A dry run

- **WHEN** neither crates.io token is set, `GITHUB_TOKEN` is set, crates.io lacks `demo`, and `verctl release bootstrap --dry-run` runs
- **THEN** it prints the release it would make and the entry it would add for `demo`, and exits zero

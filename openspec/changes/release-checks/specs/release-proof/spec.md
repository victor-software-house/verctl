## ADDED Requirements

### Requirement: `release` runs the registry checks before its first upload

Before its first write, `verctl release` SHALL run each check `release_checks` names.
`release_checks` SHALL default to `registry` and `trust`, and `trust` SHALL run only when `VERCTL_OWNER_TOKEN` is set, reported as skipped otherwise.
When a check fails, `verctl release` SHALL exit non-zero naming each failure, and SHALL create no draft and publish, tag, and release nothing.

#### Scenario: Drift between the merge and the release

- **WHEN** crate `demo` lost owner `any-victor` after the Version PR merged, and `verctl release` runs from the push that landed it
- **THEN** the `registry` check fails, the run exits non-zero naming `demo`, and no crate, tag, or Release is published

#### Scenario: No owner's token

- **WHEN** `VERCTL_OWNER_TOKEN` is unset and `verctl release` runs with the default `release_checks`
- **THEN** it runs `registry`, reports `trust` as skipped, and sends no request to `/api/v1/trusted_publishing/github_configs`

#### Scenario: The proof turned off

- **WHEN** `.ctl/ver.yaml` sets `release_checks: []`
- **THEN** `verctl release` runs no check before its first upload

### Requirement: Run by `release`, the `registry` check skips the freeze

When `verctl release` or `verctl release --dry-run` runs the `registry` check, it SHALL skip the freeze and the wait for a running release, and run only the existence and owner proofs.
The run is the pending release itself, or changes nothing, so the freeze would only hold it back; `check registry` alone carries the freeze.
`trust` SHALL run as its own check as `release_checks` names it.

#### Scenario: The landing push

- **WHEN** the Version PR's merge lands `demo` `0.4.0`, and `verctl release` runs from that push while its own workflow is the running release
- **THEN** the `registry` check proves existence and owners, does not wait for the running release, and does not fail naming `demo` `0.4.0` as pending

#### Scenario: A rerun of a partial release

- **WHEN** a release stopped after one of three crates went live, and `verctl release` runs again at the same tip
- **THEN** the `registry` check skips the freeze, does not wait on its own workflow, and the run goes on to the unlisted crates

### Requirement: The release proof repeats no earlier gate

The proof SHALL NOT replay `version`, run `cargo publish --dry-run`, or build a tarball.
The Version PR's `check versions`, CI's `cargo package` and the release workflow's asset matrix prove those before the release.

#### Scenario: A release of a crate with assets

- **WHEN** `verctl release` runs with the default `release_checks` for a crate with declared assets
- **THEN** it runs no `cargo publish --dry-run`, no replay of `version` and no asset build before its first upload

### Requirement: `release --dry-run` prints the plan and runs the same checks

`verctl release --dry-run` (`-n`) SHALL make every read `release` makes and run the checks `release` would run, SHALL fail with the message `release` would, and SHALL create no draft, upload nothing, tag nothing and change no registry.

#### Scenario: A dry run with a missing owner

- **WHEN** crate `demo` lacks owner `any-victor` and `verctl release --dry-run` runs
- **THEN** it prints the plan, the `registry` check fails naming `demo`, and it exits non-zero having created no draft

### Requirement: `release_checks` names the checks `release` runs

The top-level `release_checks` key of `.ctl/ver.yaml` SHALL list check names from `registry` and `trust`.
Omitted, it SHALL be `[registry, trust]`.
A name outside that set, or a name listed twice, SHALL fail the load and name the field.

#### Scenario: A check that runs before the merge

- **WHEN** `.ctl/ver.yaml` sets `release_checks: [versions]`
- **THEN** loading `.ctl/ver.yaml` fails, names `release_checks`, and lists `registry` and `trust`

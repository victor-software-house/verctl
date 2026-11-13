## ADDED Requirements

### Requirement: `check` runs the checks it names

`verctl check [CHECK]...` SHALL run each named check, from `fragments`, `served`, `versions`, `registry` and `trust`, and SHALL report each one's result.
Names SHALL add up and never overlap, and each check SHALL run once however often its name appears.

With no name, `check` SHALL run `fragments` and `served`.
These need only the checkout, with no network and no token.
`--all` (`-a`) SHALL run every check.
An unknown name SHALL fail the parse and list the known names.

#### Scenario: Bare check

- **WHEN** `verctl check` runs with no name and no network
- **THEN** it runs `fragments` and `served`, sends no request, and exits zero when both pass

#### Scenario: Two names

- **WHEN** `verctl check versions registry` runs in CI
- **THEN** it runs `versions` and `registry` only, and reports each one's result

#### Scenario: Every check

- **WHEN** `verctl check --all` runs with no owner's token in `VERCTL_OWNER_TOKEN`
- **THEN** it runs all five checks, and `trust` fails naming `VERCTL_OWNER_TOKEN`

#### Scenario: An unknown name

- **WHEN** `verctl check release` runs
- **THEN** it exits 2 and lists `fragments`, `served`, `versions`, `registry` and `trust`

### Requirement: `check` takes no `--versions` flag

`verctl check` SHALL NOT accept `--versions`, and SHALL NOT read `VERCTL_VERSIONS`.
The comparison of manifests with the default branch SHALL run only as the `versions` check.

#### Scenario: The removed flag

- **WHEN** `verctl check --versions` runs
- **THEN** it exits 2 with clap's unknown-argument error, and runs no check

### Requirement: The `registry` check proves the registry side without a token

`verctl check registry` SHALL prove, for each crate the repository publishes to crates.io, that the crate exists, and that its owners include every login its `owners` lists.
Without any `owners` for a crate it SHALL skip the owner proof for that crate and say so.

It SHALL report, as evidence and not as proof, whether the crate's latest version came through trusted publishing from the repository.
It SHALL exit non-zero naming each crate and each failed proof.

#### Scenario: Every crate is ready

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]`, each crate exists and lists `any-victor` as an owner, and `verctl check registry` runs with no token
- **THEN** it exits zero, and reports for each crate whether its latest version came through trusted publishing from the repository

#### Scenario: An owner is missing

- **WHEN** `.ctl/ver.yaml` sets `owners: [any-victor]` and crate `demo` lists only `alice` as an owner
- **THEN** `verctl check registry` exits non-zero, names `demo`, the missing login `any-victor`, and the owners it found

### Requirement: The `registry` check repeats every request on a rerun

The `registry` check SHALL need the network and no token.
A rerun SHALL repeat every request whatever the state of the pull request.

#### Scenario: A rerun repeats the proof

- **WHEN** `verctl check registry` failed on `demo-extra`, which the default branch holds, the operator ran `verctl release bootstrap demo-extra` at the default branch's tip, and the CI job reruns with "Re-run failed jobs"
- **THEN** the rerun queries crates.io again and exits zero

### Requirement: The `trust` check proves the trusted publisher with an owner's token

`verctl check trust` SHALL read `VERCTL_OWNER_TOKEN`, an owner's token with the `trusted-publishing` scope, and SHALL prove for each crate that a trusted publisher entry names the repository and the workflow file `release_workflow` names.
Without the token it SHALL fail and name the variable.

#### Scenario: A missing entry

- **WHEN** `VERCTL_OWNER_TOKEN` holds an owner's token, and crate `demo` has no trusted publisher entry naming `release.yml`
- **THEN** `verctl check trust` exits non-zero and names `demo` as lacking a trusted publisher for `release.yml`

#### Scenario: No token

- **WHEN** `VERCTL_OWNER_TOKEN` is unset and `verctl check trust` runs
- **THEN** it exits non-zero, names `VERCTL_OWNER_TOKEN`, and sends no request to `/api/v1/trusted_publishing/github_configs`

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

### Requirement: The release proof repeats no earlier gate

The proof SHALL NOT replay `version`, run `cargo publish --dry-run`, or build a tarball.
The Version PR's `check versions`, CI's `cargo package` and the release workflow's asset matrix prove those before the release.

#### Scenario: A release of a crate with assets

- **WHEN** `verctl release` runs with the default `release_checks` for a crate with declared assets
- **THEN** it runs no `cargo publish --dry-run`, no replay of `version` and no asset build before its first upload

### Requirement: `release --dry-run` prints the plan and runs the same checks

`verctl release --dry-run` (`-n`) SHALL print the plan and run the checks `release` would run, and SHALL upload nothing and change no registry and no Release.

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

## ADDED Requirements

### Requirement: `release --new` creates the crates the registry lacks

`verctl release --new` (`VERCTL_NEW`) SHALL let a release create each crate crates.io lacks, with the registry token from `VERCTL_REGISTRY_TOKEN`, and publish its real code at the version the default branch holds.
Every other upload SHALL use the trusted-publishing token when `VERCTL_TRUSTED_PUBLISHING_TOKEN` is set, and the registry token otherwise.
It SHALL run locally or in the release job, and the release's proofs, draft Release and tag SHALL stay as they are.

#### Scenario: A new crate

- **WHEN** `shrub-process` is at `0.7.0` on `main`, crates.io lacks it and lists the workspace's other crates, and `verctl release --new` runs with `VERCTL_REGISTRY_TOKEN` and `VERCTL_TRUSTED_PUBLISHING_TOKEN` set
- **THEN** it uploads `shrub-process` `0.7.0` with the registry token and no other crate
- **AND** it creates no tag of its own, because the draft Release's publish creates `v0.7.0` for the whole group

#### Scenario: A crate the registry lists

- **WHEN** crates.io lists `core` `0.7.0` and lacks `shrub-process`, and `verctl release --new` runs
- **THEN** it uploads only `shrub-process`

#### Scenario: A standalone package

- **WHEN** standalone package `demo` is at `0.0.0` on `main`, crates.io lacks it, and `verctl release --new` runs
- **THEN** it uploads `demo` `0.0.0`, and the draft Release's publish creates tag `v0.0.0`

### Requirement: `release --new` needs the registry token

Without `VERCTL_REGISTRY_TOKEN`, `verctl release --new` SHALL refuse to run, SHALL name the variable and the workflow secret `registry-token`, and SHALL write nothing.
`--dry-run` SHALL need no registry token and SHALL name the variable in a notice, and it SHALL need `GITHUB_TOKEN` for the reads a release makes.
A release without `--new` SHALL fail the release proof on a crate crates.io lacks, as it does today.

#### Scenario: No token

- **WHEN** `VERCTL_REGISTRY_TOKEN` is unset and `verctl release --new` runs
- **THEN** it exits non-zero, names `VERCTL_REGISTRY_TOKEN`, and writes nothing

#### Scenario: A dry run

- **WHEN** `VERCTL_REGISTRY_TOKEN` is unset, `GITHUB_TOKEN` is set and `verctl release --dry-run --new` runs
- **THEN** it plans what `verctl release --new` plans, names `VERCTL_REGISTRY_TOKEN` in a notice, and exits zero

### Requirement: The release proof passes a crate that `--new` creates

With `--new`, the release proof SHALL pass a crate crates.io lacks, in `registry` and in `trust`, with a notice that the run creates it.
The `0.0.0` rule SHALL still fail a standalone package off `0.0.0`.

#### Scenario: A crate the run creates

- **WHEN** crates.io lacks `demo-cli` and `verctl release --new` runs
- **THEN** the proof passes `demo-cli` with the notice "`demo-cli`: crates.io lacks it; this run creates it with the registry token"

### Requirement: `release trust` adds the trusted publisher entry only when it is missing

`verctl release trust [PACKAGE]...` SHALL list each crate's entries first, because crates.io accepts duplicates, and SHALL create this repository's entry only when none matches.
It SHALL report a crate whose entry matches as `already set`, and SHALL never edit or delete another entry.
`verctl release trust --dry-run` (`-n`) SHALL list what it would add, and SHALL send no `POST`.

#### Scenario: Registering

- **WHEN** crate `shrub-process` has no entry and `verctl release trust` runs
- **THEN** it reads `GET /api/v1/trusted_publishing/github_configs?crate=shrub-process`, then sends one `POST /api/v1/trusted_publishing/github_configs` naming the repository and `release.yml`, and reports `shrub-process` as added

#### Scenario: Running again

- **WHEN** crate `shrub-process` has an entry naming the repository and `release.yml`, and `verctl release trust` runs
- **THEN** it sends no `POST` and reports `shrub-process` as `already set`

#### Scenario: Another entry

- **WHEN** crate `demo` has one entry naming another repository and none naming this one, and `verctl release trust` runs
- **THEN** it adds this repository's entry and leaves the other entry unchanged

#### Scenario: A dry run

- **WHEN** crate `shrub-process` has no entry and `verctl release trust --dry-run` runs
- **THEN** it reports the entry it would add for `shrub-process`, and sends no `POST`

### Requirement: `release trust` adds the entry `check trust` proves

`verctl release trust` SHALL take no provider argument.
It SHALL read the repository from the `origin` remote and the workflow file from `release_workflow`, the file `check trust` compares, and SHALL add the entry with no environment, so a run in any environment matches it.

#### Scenario: The default workflow

- **WHEN** the `origin` remote is `github.com/acme/demo` and `.ctl/ver.yaml` sets no `release_workflow`
- **THEN** `verctl release trust` adds an entry for owner `acme`, repository `demo`, workflow `release.yml` and no environment

### Requirement: `release trust` needs an owner's token

`verctl release trust` SHALL read the owner's token from `VERCTL_OWNER_TOKEN`, because crates.io adds an entry only for a token with the `trusted-publishing` scope.
Without it, it SHALL exit non-zero naming the variable and send no request.
A crate that already holds 5 entries, the crates.io limit, SHALL fail naming the limit, and the other crates SHALL still be handled.

#### Scenario: A crate at the limit

- **WHEN** crate `demo` holds 5 entries, none naming this repository, and `verctl release trust` runs
- **THEN** it exits non-zero naming `demo` and the limit of 5, and sends no `POST` for `demo`

#### Scenario: No owner's token

- **WHEN** `VERCTL_OWNER_TOKEN` is unset and `verctl release trust` runs
- **THEN** it exits non-zero, names `VERCTL_OWNER_TOKEN`, and sends no request

### Requirement: A first publish meets the rate limits as the release does

A release that crates.io's new-crate limit stops SHALL fail with the time crates.io gives and record it in its draft Release, and a rerun SHALL resume it.
`verctl release trust` SHALL wait on a 429 until the time the `Retry-After` header gives, as seconds or as an HTTP date, and then try again.

#### Scenario: The new-crate limit

- **WHEN** `verctl release --new` uploads 6 new crates, and crates.io answers the sixth with 429 and `try again after Fri, 13 Nov 2026 15:10:00 GMT`
- **THEN** it fails naming the sixth crate and that time, leaves the draft Release and no tag, and a rerun after that time uploads the sixth

### Requirement: Nothing is published only to hold a name

verctl SHALL publish no placeholder version.
`verctl release --new` SHALL publish only a package's real code at the version the default branch holds.

#### Scenario: A name to reserve

- **WHEN** an operator runs `verctl release bootstrap demo-next --version 0.0.0` to create `demo-next` at a version its manifest does not hold
- **THEN** verctl refuses the unknown `--version` option, because no command takes a version other than the one the manifest holds

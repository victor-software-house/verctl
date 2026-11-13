## ADDED Requirements

### Requirement: `release --new` publishes the packages the registry lacks

`verctl release --new` SHALL run locally, read the registry token from `VERCTL_REGISTRY_TOKEN`, and publish the real code of each package the registry lacks, at the version the default branch holds.

1. After the uploads, it SHALL tag each published standalone package, and then run `verctl trust`.
2. A member of a shared-version group SHALL get no tag, because its tag belongs to the group's release.
3. Without `VERCTL_REGISTRY_TOKEN`, the command SHALL refuse to run and SHALL name the variable.

#### Scenario: A new crate

- **WHEN** `shrub-process` is at `0.7.0` on `main`, crates.io lacks it, `VERCTL_REGISTRY_TOKEN` holds the operator's token, and `verctl release --new` runs
- **THEN** it publishes `shrub-process` `0.7.0` from the repository's code and runs `verctl trust`
- **AND** it creates no tag, because `shrub-process` shares the workspace version and `v0.7.0` belongs to the group's release

#### Scenario: A new standalone package

- **WHEN** standalone package `demo` is at `0.0.0` on `main`, crates.io lacks it, and `verctl release --new` runs
- **THEN** it publishes `demo` `0.0.0`, creates tag `v0.0.0` at the default branch's tip, and runs `verctl trust`

#### Scenario: No token

- **WHEN** `VERCTL_REGISTRY_TOKEN` is unset and `verctl release --new` runs
- **THEN** it exits non-zero, names `VERCTL_REGISTRY_TOKEN`, and publishes nothing

#### Scenario: A package the registry already lists

- **WHEN** crates.io lists `core` `0.7.0` and lacks `shrub-process`
- **THEN** `verctl release --new` publishes only `shrub-process`

### Requirement: `trust` adds the trusted publisher entry only when it is missing

`verctl trust` SHALL list each crate's entries first, because crates.io accepts duplicates, and SHALL create this repository's entry only when none matches.
It SHALL report a crate whose entry matches as `already set`, and SHALL never edit or delete another entry.
`verctl trust --dry-run` (`-n`) SHALL list what it would add, and SHALL send no `POST`.

#### Scenario: Registering

- **WHEN** crate `shrub-process` has no entry and `verctl trust` runs
- **THEN** it reads `GET /api/v1/trusted_publishing/github_configs?crate=shrub-process`, then sends one `POST /api/v1/trusted_publishing/github_configs` naming the repository and `release.yml`, and reports `shrub-process` as added

#### Scenario: Running again

- **WHEN** crate `shrub-process` has an entry naming the repository and `release.yml`, and `verctl trust` runs
- **THEN** it sends no `POST` and reports `shrub-process` as `already set`

#### Scenario: Another entry

- **WHEN** crate `demo` has one entry naming another repository and none naming this one, and `verctl trust` runs
- **THEN** it adds this repository's entry and leaves the other entry unchanged

#### Scenario: A dry run

- **WHEN** crate `shrub-process` has no entry and `verctl trust --dry-run` runs
- **THEN** it reports the entry it would add for `shrub-process`, and sends no `POST`

### Requirement: `trust` reads its entry from the repository

`verctl trust` SHALL take no provider argument.
It SHALL read the repository from the `origin` remote, and the workflow file and environment from the one workflow in `.github/workflows/` that calls the reusable `release.yml`.
It SHALL fail and name the files when no workflow or more than one calls it.

#### Scenario: One caller workflow

- **WHEN** the `origin` remote is `github.com/acme/demo`, and only `.github/workflows/release.yml` calls the reusable `release.yml`, with `environment: crates-io`
- **THEN** `verctl trust` adds an entry for owner `acme`, repository `demo`, workflow `release.yml` and environment `crates-io`

#### Scenario: Two caller workflows

- **WHEN** both `.github/workflows/release.yml` and `.github/workflows/hotfix.yml` call the reusable `release.yml`
- **THEN** `verctl trust` exits non-zero, names both files, and sends no `POST`

### Requirement: A first publish waits on `Retry-After`

When crates.io answers a first publish or a trusted publisher registration with 429, `verctl release --new` and `verctl trust` SHALL wait until the HTTP date in the `Retry-After` header, and then try again.

#### Scenario: The new-crate limit

- **WHEN** `verctl release --new` publishes 6 new crates, and crates.io answers the sixth with 429 and `Retry-After: Fri, 13 Nov 2026 15:10:00 GMT`
- **THEN** it reports the wait, tries the sixth again at that time, and publishes all 6 in one run

### Requirement: A repository may opt in to a token for new crates in CI

The reusable `release.yml` SHALL declare a boolean input `publish-new` defaulting to `false`, beside `publish` and `crates-io-trusted-publishing`.
With `publish-new: true` and the `registry-token` secret set, the workflow SHALL publish a crate that the registry lacks.
Trusted publishing SHALL stay the default.
With the opt-in, the `registry` check SHALL count a missing crate as one the run can create.

#### Scenario: The default

- **WHEN** a caller passes no `publish-new` input and crates.io lacks `demo`
- **THEN** the workflow publishes no token upload, and `verctl check registry` reports `demo` as missing

#### Scenario: The opt-in

- **WHEN** a caller passes `publish-new: true` with the `registry-token` secret and crates.io lacks `demo`
- **THEN** `verctl check registry` does not report `demo` as missing, and the release run publishes `demo` with that token

#### Scenario: The opt-in without a secret

- **WHEN** a caller passes `publish-new: true` and no `registry-token` secret
- **THEN** the workflow fails before any upload and names `registry-token`

### Requirement: Nothing is published only to hold a name

verctl SHALL publish no placeholder version.
`verctl release --new` SHALL publish only a package's real code at the version the default branch holds.

#### Scenario: A name to reserve

- **WHEN** an operator runs `verctl release --new --version 0.0.0` to publish `demo-next` at a version its manifest does not hold
- **THEN** verctl refuses the unknown `--version` option, because no command takes a version other than the one the manifest holds

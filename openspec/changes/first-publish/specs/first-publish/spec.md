## ADDED Requirements

### Requirement: `release bootstrap` publishes the packages the registry lacks

`verctl release bootstrap [PACKAGE]...` SHALL run locally, or in the release job when the `bootstrap` input is set.
It SHALL read the registry token from `VERCTL_REGISTRY_TOKEN`, and publish the real code of each named package the registry lacks, or of every such declared package when none is named, at the version the default branch holds.

#### Scenario: A new crate

- **WHEN** `shrub-process` is at `0.7.0` on `main`, crates.io lacks it, `VERCTL_REGISTRY_TOKEN` holds the operator's token, and `verctl release bootstrap` runs
- **THEN** it publishes `shrub-process` `0.7.0` from the repository's code and runs `verctl release trust`
- **AND** it creates no tag, because `shrub-process` shares the workspace version and `v0.7.0` belongs to the group's release

#### Scenario: A package the registry already lists

- **WHEN** crates.io lists `core` `0.7.0` and lacks `shrub-process`
- **THEN** `verctl release bootstrap` publishes only `shrub-process`

#### Scenario: A named package the registry lists

- **WHEN** crates.io lists `core` `0.7.0` and `verctl release bootstrap core` runs
- **THEN** it publishes nothing, reports `core` as already published, and exits zero

### Requirement: `release bootstrap` tags each standalone package

Run on its own, `verctl release bootstrap` SHALL tag each published standalone package after its uploads, and then run `verctl release trust` for the same packages.
A member of a shared-version group SHALL get no tag of its own.

#### Scenario: A new standalone package

- **WHEN** standalone package `demo` is at `0.0.0` on `main`, crates.io lacks it, and `verctl release bootstrap` runs
- **THEN** it publishes `demo` `0.0.0`, creates tag `v0.0.0` at the default branch's tip, and runs `verctl release trust`

### Requirement: `release bootstrap` needs the operator's token

Without `VERCTL_REGISTRY_TOKEN`, `verctl release bootstrap` SHALL refuse to run, SHALL name the variable, and SHALL publish nothing.
Run on its own, it SHALL also refuse without `VERCTL_OWNER_TOKEN`, which its `release trust` step needs, so no crate is created without its entry.
It SHALL run at the default branch's tip: after fetching the default branch, HEAD SHALL be that commit, or it SHALL exit non-zero naming both commits and publish nothing.

#### Scenario: No token

- **WHEN** `VERCTL_REGISTRY_TOKEN` is unset and `verctl release bootstrap` runs
- **THEN** it exits non-zero, names `VERCTL_REGISTRY_TOKEN`, and publishes nothing

#### Scenario: HEAD behind the default branch

- **WHEN** `origin/main` is one commit ahead of HEAD and `verctl release bootstrap` runs
- **THEN** it exits non-zero, names both commits, and publishes nothing

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

### Requirement: A first publish waits on `Retry-After`

When crates.io answers a first publish or a trusted publisher registration with 429, `verctl release bootstrap` and `verctl release trust` SHALL wait until the time crates.io gives, and then try again.
A first publish reads it from cargo's "try again after" message, and a registration from the `Retry-After` header, as seconds or as an HTTP date.

#### Scenario: The new-crate limit

- **WHEN** `verctl release bootstrap` publishes 6 new crates, and crates.io answers the sixth with 429 and `Retry-After: Fri, 13 Nov 2026 15:10:00 GMT`
- **THEN** it reports the wait, tries the sixth again at that time, and publishes all 6 in one run

### Requirement: Nothing is published only to hold a name

verctl SHALL publish no placeholder version.
`verctl release bootstrap` SHALL publish only a package's real code at the version the default branch holds.

#### Scenario: A name to reserve

- **WHEN** an operator runs `verctl release bootstrap demo-next --version 0.0.0` to publish `demo-next` at a version its manifest does not hold
- **THEN** verctl refuses the unknown `--version` option, because no command takes a version other than the one the manifest holds

## ADDED Requirements

### Requirement: `release --first` publishes the packages the registry lacks

`verctl release --first` SHALL run locally, read the registry token from `VERCTL_REGISTRY_TOKEN`, and publish the real code of each package the registry lacks, at the version the default branch holds.
It SHALL then tag each published standalone package, and then run `verctl trust`.
It SHALL create no tag for a member of a shared-version group, whose tag belongs to the group's release.
It SHALL refuse to run without `VERCTL_REGISTRY_TOKEN`, and SHALL name the variable.

#### Scenario: A new crate

- **WHEN** `shrub-process` is at `0.7.0` on `main`, crates.io lacks it, `VERCTL_REGISTRY_TOKEN` holds the operator's token, and `verctl release --first` runs
- **THEN** it publishes `shrub-process` `0.7.0` from the repository's code and runs `verctl trust`
- **AND** it creates no tag, because `shrub-process` shares the workspace version and `v0.7.0` belongs to the group's release

#### Scenario: A new standalone package

- **WHEN** standalone package `demo` is at `0.0.0` on `main`, crates.io lacks it, and `verctl release --first` runs
- **THEN** it publishes `demo` `0.0.0`, creates tag `v0.0.0` at the default branch's tip, and runs `verctl trust`

#### Scenario: No token

- **WHEN** `VERCTL_REGISTRY_TOKEN` is unset and `verctl release --first` runs
- **THEN** it exits non-zero, names `VERCTL_REGISTRY_TOKEN`, and publishes nothing

#### Scenario: A package the registry already lists

- **WHEN** crates.io lists `core` `0.7.0` and lacks `shrub-process`
- **THEN** `verctl release --first` publishes only `shrub-process`

### Requirement: `trust` registers or checks the trusted publisher

`verctl trust` SHALL register the repository's release workflow as each crate's trusted publisher through `POST /api/v1/trusted_publishing/github_configs`, and SHALL skip a crate that already lists it.
`verctl trust --check` SHALL only report each crate's state, and SHALL send no `POST`.

#### Scenario: Registering

- **WHEN** crate `shrub-process` has no trusted publisher and `verctl trust` runs
- **THEN** it sends `POST /api/v1/trusted_publishing/github_configs` naming the repository and `release.yml`, and reports `shrub-process` as registered

#### Scenario: Checking only

- **WHEN** crate `shrub-process` has no trusted publisher and `verctl trust --check` runs
- **THEN** it exits non-zero, names `shrub-process`, and sends no `POST`

### Requirement: A first publish waits on `Retry-After`

When crates.io answers a first publish or a trusted publisher registration with 429, `verctl release --first` and `verctl trust` SHALL wait until the HTTP date in the `Retry-After` header, and then try again.

#### Scenario: The new-crate limit

- **WHEN** `verctl release --first` publishes 6 new crates, and crates.io answers the sixth with 429 and `Retry-After: Fri, 13 Nov 2026 15:10:00 GMT`
- **THEN** it reports the wait, tries the sixth again at that time, and publishes all 6 in one run

### Requirement: A repository may opt in to a token for first publishes in CI

The reusable `release.yml` SHALL declare a boolean input `first-publish` defaulting to `false`.
With `first-publish: true` and the `registry-token` secret set, the workflow SHALL publish a crate that the registry lacks.
Trusted publishing SHALL stay the default.
With the opt-in, `check --registry` SHALL count a missing crate as one the run can create.

#### Scenario: The default

- **WHEN** a caller passes no `first-publish` input and crates.io lacks `demo`
- **THEN** the workflow publishes no token upload, and `verctl check --registry` reports `demo` as missing

#### Scenario: The opt-in

- **WHEN** a caller passes `first-publish: true` with the `registry-token` secret and crates.io lacks `demo`
- **THEN** `verctl check --registry` does not report `demo` as missing, and the release run publishes `demo` with that token

#### Scenario: The opt-in without a secret

- **WHEN** a caller passes `first-publish: true` and no `registry-token` secret
- **THEN** the workflow fails before any upload and names `registry-token`

### Requirement: Nothing is published only to hold a name

verctl SHALL publish no placeholder version.
`verctl release --first` SHALL publish only a package's real code at the version the default branch holds.

#### Scenario: A name to reserve

- **WHEN** an operator runs `verctl release --first --version 0.0.0` to publish `demo-next` at a version its manifest does not hold
- **THEN** verctl refuses the unknown `--version` option, because no command takes a version other than the one the manifest holds

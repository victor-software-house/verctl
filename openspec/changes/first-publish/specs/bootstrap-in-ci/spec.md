## ADDED Requirements

### Requirement: A repository may opt in to a token for new crates in CI

The reusable `release.yml` SHALL declare a boolean input `bootstrap` defaulting to `false`, beside `publish` and `assets`.
With `bootstrap: true` and the `registry-token` secret set, the release job SHALL run `verctl release --bootstrap`, which creates each package the registry lacks with that token.
Trusted publishing SHALL stay the default.
With the opt-in, the release proof's `registry` check SHALL count a missing crate as one the run can create.

#### Scenario: The default

- **WHEN** a caller passes no `bootstrap` input and crates.io lacks `demo`
- **THEN** the workflow publishes no token upload, and `verctl check registry` reports `demo` as missing

#### Scenario: The opt-in

- **WHEN** a caller passes `bootstrap: true` with the `registry-token` secret and crates.io lacks `demo`
- **THEN** the release proof passes `demo` with a notice that the run creates it, and the release run publishes `demo` with that token

#### Scenario: The opt-in without a secret

- **WHEN** a caller passes `bootstrap: true` and no `registry-token` secret
- **THEN** the workflow fails before any upload and names `registry-token`

### Requirement: Inside a release run, bootstrap only uploads

Inside a release run, `verctl release --bootstrap` SHALL only upload.
The run SHALL plan each standalone package at `0.0.0` that the registry lists and that has no tag, whichever attempt bootstrapped it.
The publish of the run's draft Release SHALL create that tag, as release-order requires.

#### Scenario: Bootstrap inside a release run

- **WHEN** the release job runs with `bootstrap: true`, standalone package `demo` at `0.0.0` is missing from crates.io, and the next registry upload of the run fails
- **THEN** `demo` `0.0.0` is on crates.io and in the run's plan, no tag `v0.0.0` exists, and the run's draft Release stays a draft until a rerun publishes it with tag `v0.0.0`

### Requirement: A dry run with the opt-in needs no token

`verctl release --dry-run --bootstrap` SHALL plan what `verctl release --bootstrap` plans, and SHALL need no token.
Without `VERCTL_REGISTRY_TOKEN` it SHALL name that variable in a notice, and only a real run SHALL fail.

#### Scenario: A dry run without the token

- **WHEN** `VERCTL_REGISTRY_TOKEN` is unset, standalone package `demo` at `0.0.0` is missing from crates.io, and `verctl release --dry-run --bootstrap` runs
- **THEN** it plans `demo` `0.0.0` with tag `v0.0.0`, names `VERCTL_REGISTRY_TOKEN` in a notice, and exits zero

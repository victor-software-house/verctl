## ADDED Requirements

### Requirement: A repository may opt in to a token for new crates in CI

The reusable `release.yml` SHALL declare a boolean input `publish-new` defaulting to `false`, beside `publish` and `assets`.
With `publish-new: true` and the `registry-token` secret set, the release job SHALL run `verctl release --new`, which creates each package the registry lacks with that token.
Trusted publishing SHALL stay the default.

#### Scenario: The default

- **WHEN** a caller passes no `publish-new` input and crates.io lacks `demo`
- **THEN** the workflow publishes no token upload, and `verctl check registry` reports `demo` as missing

#### Scenario: The opt-in

- **WHEN** a caller passes `publish-new: true` with the `registry-token` secret and crates.io lacks `demo`
- **THEN** the release proof passes `demo` with a notice that the run creates it, and the release run publishes `demo` with that token

#### Scenario: The opt-in without a secret

- **WHEN** a caller passes `publish-new: true` and no `registry-token` secret
- **THEN** the workflow fails before any upload and names `registry-token`

### Requirement: The opt-in combines with trusted publishing

`crates-io-trusted-publishing` and `publish-new` SHALL combine, and the release proof's `registry` check SHALL count a missing crate as one the run can create.

#### Scenario: Both inputs

- **WHEN** a caller passes both inputs with the `registry-token` secret, crates.io lacks `demo-cli`, and lists `demo-core` without this version
- **THEN** the release uploads `demo-cli` with the registry token and `demo-core` with the trusted-publishing token

### Requirement: A release run plans a new standalone package

The run SHALL plan each standalone package at `0.0.0` that the registry lists and that has no tag, and, with `--new`, each that the registry lacks.
The publish of the run's draft Release SHALL create that tag, as release-order requires.
The plan job SHALL read the same `--new` input as the publish job, so both plan the same packages.

#### Scenario: A new package inside a release run

- **WHEN** the release job runs with `publish-new: true`, standalone package `demo` at `0.0.0` is missing from crates.io, and the next registry upload of the run fails
- **THEN** `demo` `0.0.0` is on crates.io and in the run's plan, no tag `v0.0.0` exists, and the run's draft Release stays a draft until a rerun publishes it with tag `v0.0.0`

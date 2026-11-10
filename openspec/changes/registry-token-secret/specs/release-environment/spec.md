## RENAMED Requirements

- FROM: `### Requirement: Inside an environment the token is read under the stored name`
- TO: `### Requirement: The caller maps the token by name`

## MODIFIED Requirements

### Requirement: The caller maps the token by name

The `publish` job SHALL read the registry token only from the `registry-token` secret, and SHALL NOT read any secret by a computed name.
When `environment` is set, the `publish` job SHALL receive the environment's value of the secret the caller maps to `registry-token`.

#### Scenario: Token kept in the environment

- **WHEN** a caller passes `environment: release` and maps `registry-token: ${{ secrets.CARGO_REGISTRY_TOKEN }}`, and only the `release` environment holds `CARGO_REGISTRY_TOKEN`
- **THEN** `verctl publish` runs with `CARGO_REGISTRY_TOKEN` set to the environment's value

#### Scenario: No environment, repository secret present

- **WHEN** a caller passes `secrets: inherit` and no `environment`, and the repository holds `CARGO_REGISTRY_TOKEN`
- **THEN** the `publish` job reads no registry token, as before

#### Scenario: Trusted publishing in an environment that holds a token

- **WHEN** a caller passes `environment: release` and `crates-io-trusted-publishing: true`, maps no `registry-token`, and the `release` environment holds `CARGO_REGISTRY_TOKEN`
- **THEN** the `publish` job publishes with the trusted-publishing token and does not fail the "not both" guard

#### Scenario: A token reached only through secrets: inherit

- **WHEN** a caller passes `environment: release` and `secrets: inherit`, maps no `registry-token`, and the `release` environment holds `CARGO_REGISTRY_TOKEN`
- **THEN** the `publish` job reads no registry token

## ADDED Requirements

### Requirement: verctl reads the registry token once

`verctl release` SHALL read the registry token from `VERCTL_REGISTRY_TOKEN` or `VERCTL_TRUSTED_PUBLISHING_TOKEN`, and SHALL fail before any other work when both are set.
A `--registry-token-env` value, or a `VERCTL_REGISTRY_TOKEN_ENV` value, that is not a variable name SHALL fail the parse with exit code 2.
The publish step of the reusable `release.yml` SHALL set these three variables in `env:` and run `mise run ver release`.

#### Scenario: Both tokens set

- **WHEN** `verctl release` runs with `VERCTL_REGISTRY_TOKEN` and `VERCTL_TRUSTED_PUBLISHING_TOKEN` both set
- **THEN** it fails before reading `.ctl/ver.yaml`, naming both variables and neither token

#### Scenario: A name a shell cannot export

- **WHEN** `verctl release --registry-token-env 1TOKEN` runs, or `VERCTL_REGISTRY_TOKEN_ENV=CARGO-TOKEN verctl release`
- **THEN** it exits 2 and says the value is not a variable name

### Requirement: Only the publisher and its query run with the registry token

`verctl release` SHALL run each publisher, and the registry `query` that checks it, with the token under the variable `--registry-token-env` names.
It SHALL start no other command with the token, and SHALL start no command with either source variable.

#### Scenario: The publisher reads its own variable

- **WHEN** `VERCTL_REGISTRY_TOKEN` is set and `--registry-token-env CARGO_REGISTRIES_DEMO_TOKEN` is given
- **THEN** the publisher and its `query` run with `CARGO_REGISTRIES_DEMO_TOKEN` set to the token

#### Scenario: A build sees no token

- **WHEN** `verctl release asset linux-x64 --dry-run` runs with `VERCTL_REGISTRY_TOKEN` and `VERCTL_TRUSTED_PUBLISHING_TOKEN` set
- **THEN** the asset build's environment holds neither variable

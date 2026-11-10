## RENAMED Requirements

- FROM: `### Requirement: Inside an environment the token is read under its stored name`
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

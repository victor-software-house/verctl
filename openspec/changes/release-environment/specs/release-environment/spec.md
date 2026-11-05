## ADDED Requirements

### Requirement: The publish job runs in the caller's environment

`release.yml` SHALL accept an `environment` string input, default empty, and SHALL set it as the `publish` job's `environment`.
No other job SHALL take it.

#### Scenario: Named environment

- **WHEN** a caller passes `environment: release`
- **THEN** the `publish` job runs in the `release` environment and the `plan`, `assets`, `docs`, and `propose` jobs run in none

#### Scenario: Empty environment

- **WHEN** a caller passes no `environment`
- **THEN** the `publish` job runs in no environment and records no deployment

### Requirement: Inside an environment the token is read under its stored name

When `environment` is set and the `registry-token` secret is empty, the `publish` job SHALL read the secret named by `registry-token-env`.
When `environment` is empty, it SHALL read only `registry-token`.

#### Scenario: Token kept in the environment

- **WHEN** a caller passes `environment: release` and `secrets: inherit`, and the `release` environment holds `CARGO_REGISTRY_TOKEN`
- **THEN** `verctl publish` runs with `CARGO_REGISTRY_TOKEN` set to the environment's value

#### Scenario: No environment, repository secret present

- **WHEN** a caller passes `secrets: inherit` and no `environment`, and the repository holds `CARGO_REGISTRY_TOKEN`
- **THEN** the `publish` job reads no registry token, as before

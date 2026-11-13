## ADDED Requirements

### Requirement: Every release job runs the verctl the plan job checked

The plan job of `release.yml` SHALL output the verctl version it runs at HEAD as `verctl-version`.
The publish, asset, and docs jobs SHALL run `verctl` at that version, whatever verctl the checked-out version commit pins.
A run that passes `verctl-artifact` SHALL run that build in every job.

#### Scenario: An old version commit pins an older verctl

- **WHEN** HEAD pins verctl 0.8.1, the pending version commit pins verctl 0.7.4, and the release workflow runs
- **THEN** the publish job runs verctl 0.8.1 through `mise exec -- "$VERCTL_BIN" release` and does not run 0.7.4

#### Scenario: verctl's own release

- **WHEN** `self-release.yml` passes `verctl-artifact`
- **THEN** the publish job runs `mise exec -- verctl release` with this run's build first on PATH

### Requirement: The plan's verctl installs from HEAD's lock

The publish, asset, and docs jobs SHALL install the plan's verctl with `mise -C .verctl-head install --locked`, where `.verctl-head` is a checkout of HEAD's mise config files, locks, and `.miserc.toml`.
The step SHALL fail when the installed binary is missing or does not run `--version`.
No job SHALL set `MISE_LOCKED` or `MISE_LOCKFILE`, and none SHALL write a lock into the version commit's checkout.

#### Scenario: A caller that locks every tool

- **WHEN** the caller's `mise.toml` sets `[settings] locked = true` and `[tool_config] locked = true`, and the version commit's `mise.lock` lacks the plan's verctl
- **THEN** the publish job installs that verctl from HEAD's `mise.lock` and the step does not fail with `locked mode requires lockfile to be enabled`

#### Scenario: HEAD's lock lacks the version

- **WHEN** HEAD's lock does not list the plan's verctl for the runner's platform
- **THEN** `mise -C .verctl-head install --locked` fails and the job stops before any release step

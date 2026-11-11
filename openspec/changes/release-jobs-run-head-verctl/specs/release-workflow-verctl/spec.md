## ADDED Requirements

### Requirement: Every release job runs the verctl the plan job checked

The plan job of `release.yml` SHALL output the verctl version it runs at HEAD as `verctl-version`.
The publish, asset, and docs jobs SHALL run `verctl` at that version, whatever verctl the checked-out version commit pins.
A run that passes `verctl-artifact` SHALL run that build in every job.

#### Scenario: An old version commit pins an older verctl

- **WHEN** HEAD pins verctl 0.8.1, the pending version commit pins verctl 0.7.4, and the release workflow runs
- **THEN** the publish job runs `mise exec github:victor-software-house/verctl@0.8.1 -- verctl release` and does not run 0.7.4

#### Scenario: verctl's own release

- **WHEN** `self-release.yml` passes `verctl-artifact`
- **THEN** the publish job runs `mise exec -- verctl release` with this run's build first on PATH

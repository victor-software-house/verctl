## ADDED Requirements

### Requirement: The default branch freezes while a release is incomplete

While the default branch holds a version whose release is incomplete, `verctl check registry` SHALL fail on every other pull request or merge group, and SHALL name the pending release.
It SHALL first wait for a running release workflow run at the default branch's tip, within its timeout, and SHALL decide after that run ends.

#### Scenario: A pull request while a release is partial

- **WHEN** `main` holds `demo` `0.4.0`, crates.io does not list it, no release run is active, and `verctl check registry` runs on another pull request
- **THEN** it exits non-zero and names `demo@0.4.0` as the pending release

#### Scenario: A pull request during a normal release

- **WHEN** `main` holds `demo` `0.4.0`, a `release.yml` run at the tip of `main` is in progress, and `verctl check registry` runs on another pull request
- **THEN** it waits for that run
- **AND** when the run ends with `demo` `0.4.0` listed on crates.io and tag `v0.4.0` published, the check exits zero

#### Scenario: The wait runs out

- **WHEN** the `release.yml` run at the tip of `main` is still running when the timeout ends
- **THEN** `verctl check registry` exits non-zero and names the running release and the timeout

### Requirement: The `verctl: unfreeze` label passes the freeze

A pull request that carries the label `verctl: unfreeze` SHALL pass the freeze.
The label SHALL pass only the freeze; every other proof of the `registry` check SHALL still apply.

#### Scenario: A release that can never complete

- **WHEN** crate `demo` `0.4.0` can never publish, because another owner took the name, and a pull request labelled `verctl: unfreeze` renames the package to `demo-cli` at `0.0.0`, crates.io lacks `demo-cli`, and the pull request runs `verctl check registry`
- **THEN** the freeze does not fail the check, and the check names the pending release as passed by the label
- **AND** it runs no proof for `demo`, which the checked commit no longer holds, and gives `demo-cli` the bootstrap notice

#### Scenario: The label does not pass a registry proof

- **WHEN** a pull request labelled `verctl: unfreeze` runs `verctl check registry`, and crate `demo` lists only `alice` as an owner where `owners` lists `any-victor`
- **THEN** `verctl check registry` exits non-zero naming `demo` and the missing login `any-victor`

### Requirement: `status` names the enforcement tier

`verctl status` SHALL name the enforcement tier, read from the default branch's rules, as one of these:

1. `merge queue`: a merge queue applies to the default branch
2. `rulesets without a queue`: rulesets apply and no merge queue does
3. `neither`: no ruleset or merge queue applies, and the freeze is reported, not enforced

#### Scenario: A merge queue

- **WHEN** the default branch's rules include a merge queue and `verctl status` runs
- **THEN** it reports the tier `merge queue`

#### Scenario: Rulesets without a queue

- **WHEN** the default branch's rules include a ruleset requiring the `release --dry-run` job on pull requests and no merge queue
- **THEN** `verctl status` reports `rulesets without a queue`

#### Scenario: Neither

- **WHEN** the default branch has no ruleset and no merge queue
- **THEN** `verctl status` reports `neither` and states that the freeze is not enforced

## ADDED Requirements

### Requirement: The config file names the Version PR

`verctl prepare --pr` SHALL take the Version PR title from `proposal.title`
in `.ctl/ver.yaml` and the commit message from `proposal.commit_message`.
An unset or empty `title` SHALL be `chore(release): version packages`.
An unset or empty `commit_message` SHALL be the resolved title.

#### Scenario: Neither key set

- **WHEN** `.ctl/ver.yaml` has no `proposal` block and `verctl prepare --pr` opens the Version PR
- **THEN** the pull request title is `chore(release): version packages`
- **AND** the commit on `verctl/release/main` has the message `chore(release): version packages`

#### Scenario: Only the title set

- **WHEN** `.ctl/ver.yaml` sets `proposal: {title: "chore: release"}` and `verctl prepare --pr` runs
- **THEN** the pull request title and the commit message are both `chore: release`

### Requirement: The title variables are not read

verctl SHALL NOT read `VERCTL_PR_TITLE` or `VERCTL_COMMIT_MESSAGE`.

#### Scenario: A variable left in a job

- **WHEN** `VERCTL_PR_TITLE=custom` is set, `.ctl/ver.yaml` has no `proposal` block, and `verctl prepare --pr` runs
- **THEN** the pull request title is `chore(release): version packages`

### Requirement: The workflow and the action take no title inputs

The reusable `release.yml` and `actions/version-pr` SHALL declare no `title`
and no `commit-message` input.

#### Scenario: A caller still passes the input

- **WHEN** a caller workflow passes `title: chore: release` to `release.yml` at the 0.8.0 tag
- **THEN** GitHub refuses the call because the input is not declared

### Requirement: The Version PR has no label

`verctl prepare --pr` SHALL apply no label to the Version PR. A
`.ctl/ver.yaml` that sets `prepare.version_label` SHALL fail to load with a
message that names the key and says to delete it.

#### Scenario: A file that still sets the label

- **WHEN** `.ctl/ver.yaml` sets `prepare: {version_label: release}` and `verctl status` runs
- **THEN** it exits non-zero before reading any fragment
- **AND** the message names `prepare.version_label` and says the Version PR is no longer labeled

#### Scenario: Another unknown key under prepare

- **WHEN** `.ctl/ver.yaml` sets `prepare: {stages: [docs]}`
- **THEN** loading it fails and the message names `prepare.stages` on its line

### Requirement: The workflow refuses an older verctl

Each job of the reusable `release.yml` that runs verctl SHALL first compare
`verctl --version` with the minimum the workflow names. It SHALL fail when
the installed version is lower, and the failure SHALL name both versions.

#### Scenario: A caller pins verctl 0.7.4

- **WHEN** a caller uses `release.yml` from 0.8.0 and its `mise.lock` pins verctl 0.7.4
- **THEN** the job fails before any verctl verb runs
- **AND** the error names 0.7.4, the minimum 0.8.0, and says to move the verctl pin and the workflow ref together

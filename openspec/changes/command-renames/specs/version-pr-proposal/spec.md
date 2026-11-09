## MODIFIED Requirements

### Requirement: The config file names the Version PR

`verctl version --pr` SHALL take the Version PR title from `proposal.title`
in `.ctl/ver.yaml` and the commit message from `proposal.commit_message`.
An unset or empty `title` SHALL be `chore(release): version packages`.
An unset or empty `commit_message` SHALL be the resolved title.

#### Scenario: Neither key set

- **WHEN** `.ctl/ver.yaml` has no `proposal` block and `verctl version --pr` opens the Version PR
- **THEN** the pull request title is `chore(release): version packages`
- **AND** the commit on `verctl/release/main` has the message `chore(release): version packages`

#### Scenario: Only the title set

- **WHEN** `.ctl/ver.yaml` sets `proposal: {title: "chore: release"}` and `verctl version --pr` runs
- **THEN** the pull request title and the commit message are both `chore: release`

### Requirement: The title variables are not read

verctl SHALL NOT read `VERCTL_PR_TITLE` or `VERCTL_COMMIT_MESSAGE`.

#### Scenario: A variable left in a job

- **WHEN** `VERCTL_PR_TITLE=custom` is set, `.ctl/ver.yaml` has no `proposal` block, and `verctl version --pr` runs
- **THEN** the pull request title is `chore(release): version packages`

### Requirement: The Version PR has no label

`verctl version --pr` SHALL apply no label to the Version PR. A
`.ctl/ver.yaml` that sets `prepare.version_label` SHALL fail to load with a
message that names the key and says to delete it.

#### Scenario: A file that still sets the label

- **WHEN** `.ctl/ver.yaml` sets `prepare: {version_label: release}` and `verctl status` runs
- **THEN** it exits non-zero before reading any fragment
- **AND** the message names `prepare.version_label` and says the Version PR is no longer labeled

#### Scenario: Another unknown key under prepare

- **WHEN** `.ctl/ver.yaml` sets `prepare: {stages: [docs]}`
- **THEN** loading it fails and the message names `prepare.stages` on its line

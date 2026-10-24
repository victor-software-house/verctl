## ADDED Requirements

### Requirement: Every long option reads a VERCTL variable

verctl SHALL bind every long option of every command to `VERCTL_<LONG>`,
through ctl-core's `env_prefix`. A flag on the command line SHALL win over
its variable, and an empty variable SHALL count as unset.

#### Scenario: JSON from the environment

- **WHEN** `VERCTL_FORMAT=json` is set and a user runs `verctl status`
- **THEN** stdout is one JSON document

#### Scenario: The flag wins

- **WHEN** `VERCTL_FORMAT=json` is set and a user runs `verctl status --format pretty`
- **THEN** stdout is the pretty report, not JSON

#### Scenario: An empty config variable

- **WHEN** `VERCTL_CONFIG` is set to the empty string and a user runs `verctl prepare --dry-run`
- **THEN** verctl reads `.ctl/ver.yaml`

### Requirement: The actions run the bare verb

The `version-pr`, `publish`, and `asset` actions SHALL pass every setting as
a `VERCTL_*` variable and run `verctl` with no argument built in the shell.
Their `config` input SHALL default to empty.

#### Scenario: A caller that sets nothing

- **WHEN** a workflow uses `actions/publish` with no inputs
- **THEN** the step runs `exec verctl publish` with `VERCTL_CONFIG` empty, and verctl reads `.ctl/ver.yaml`

#### Scenario: A dry run

- **WHEN** a workflow uses `actions/version-pr` with `dry-run: true`
- **THEN** the step runs `exec verctl prepare --pr` with `VERCTL_DRY_RUN=true`, and verctl writes nothing

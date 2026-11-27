## ADDED Requirements

### Requirement: Piped output defaults to tsv

When stdout is not a terminal and no flag or variable names a format, verctl SHALL write `tsv`.
A JSON failure SHALL go to stderr, and stdout SHALL stay empty after a failed run.

#### Scenario: Status piped

- **WHEN** `verctl status | cat` runs with no `--format` and no `VERCTL_FORMAT`
- **THEN** each line holds tab-separated columns and no box-drawing characters

#### Scenario: A JSON failure

- **WHEN** `verctl status --config missing.yaml --format json` fails
- **THEN** the error envelope is on stderr and stdout is empty

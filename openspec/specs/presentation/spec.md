# presentation Specification

## Purpose

How verctl presents release reports on top of the ctl-core presentation kernel: which values take the identifier role, and how records and JSON render.

## Requirements

### Requirement: Records render borderless

verctl SHALL render key and value summaries with ctl-core's default record
style: no frame and right-aligned keys.

#### Scenario: Publish dry run

- **WHEN** `verctl publish --dry-run` runs for one cargo crate at version 0.0.1
- **THEN** the summary lines read `release  would create v0.0.1` and `dry-run  nothing published`
- **AND** they carry no box-drawing characters

### Requirement: Package names use the identifier role

verctl SHALL render package names through ctl-core's identifier role and keep
build targets and check names as tokens.

#### Scenario: Status table

- **WHEN** `verctl status --color always` lists one fragment for package `demo`
- **THEN** `demo` is bold with no colour escape

### Requirement: Piped output defaults to tsv

When stdout is not a terminal and no flag or variable names a format, verctl SHALL write `tsv`.
A JSON failure SHALL go to stderr, and stdout SHALL stay empty after a failed run.

#### Scenario: Status piped

- **WHEN** `verctl status | cat` runs with no `--format` and no `VERCTL_FORMAT`
- **THEN** each line holds tab-separated columns and no box-drawing characters

#### Scenario: A JSON failure

- **WHEN** `verctl status --config missing.yaml --format json` fails
- **THEN** the error envelope is on stderr and stdout is empty

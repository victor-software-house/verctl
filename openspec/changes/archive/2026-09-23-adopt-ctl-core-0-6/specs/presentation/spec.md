# Presentation

## ADDED Requirements

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

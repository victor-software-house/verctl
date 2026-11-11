# `version-pr-auto-merge` Specification

## Purpose

Who merges the Version PR: a person, never verctl.

## Requirements

### Requirement: verctl never arms a Version PR's merge

`verctl prepare --pr` SHALL open or update the Version PR and SHALL NOT enable GitHub auto-merge on it.
`prepare` SHALL refuse `--auto-merge` as an unknown argument, and neither the reusable release workflow nor `actions/version-pr` SHALL declare an `auto-merge` input.

#### Scenario: The removed flag

- **WHEN** an operator runs `verctl prepare --pr --auto-merge`
- **THEN** verctl exits non-zero with clap's unexpected-argument error before it reads a fragment or writes a file

#### Scenario: A caller that still passes the input

- **WHEN** a caller workflow passes `auto-merge: true` to `victor-software-house/verctl/.github/workflows/release.yml` at this release
- **THEN** GitHub refuses the run because the workflow declares no `auto-merge` input

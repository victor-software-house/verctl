## ADDED Requirements

### Requirement: verctl never arms a Version PR's merge

`verctl prepare --pr` SHALL open or update the Version PR and SHALL NOT enable GitHub auto-merge on it.
`prepare` SHALL refuse `--auto-merge` as an unknown argument, and neither the reusable release workflow nor `actions/version-pr` SHALL declare an `auto-merge` input.

#### Scenario: The removed flag

- **WHEN** an operator runs `verctl prepare --pr --auto-merge`
- **THEN** verctl exits non-zero with clap's unexpected-argument error before it reads a fragment or writes a file

#### Scenario: A caller that still passes the input

- **WHEN** a caller workflow passes `auto-merge: true` to `victor-software-house/verctl/.github/workflows/release.yml` at this release
- **THEN** GitHub refuses the run because the workflow declares no `auto-merge` input

## REMOVED Requirements

### Requirement: The Version PR merges itself when a repository opts in

**Reason**: The operator decided on 2026-11-02 that a release starts only when a person merges the Version PR.

**Migration**: Drop `auto-merge` from the caller's `with:` block and merge the Version PR by hand.

### Requirement: Auto-merge needs a required check on the base branch

**Reason**: verctl no longer enables auto-merge, so there is nothing for the check to gate.

**Migration**: None. Keep the base branch's required checks for the Version PR's own CI.

### Requirement: Auto-merge needs a token other than GITHUB_TOKEN

**Reason**: verctl no longer enables auto-merge, so it no longer compares its token with `VERCTL_JOB_TOKEN`.

**Migration**: Keep the App secrets: a Version PR opened with `GITHUB_TOKEN` still starts no CI.

## ADDED Requirements

### Requirement: The release commit is on the default branch

`verctl publish` SHALL refuse, before any publish, tag, or Release call, when
the release commit is not an ancestor of the default branch. It SHALL also
refuse when the check cannot run because there is no repository, no HEAD,
or no default-branch ref, and SHALL name which. `--allow-off-default-branch`
SHALL let the run continue.

#### Scenario: No default-branch ref

- **WHEN** the clone has no `origin/main`, `VERCTL_DEFAULT_BRANCH` is unset, and `verctl publish` runs
- **THEN** it exits non-zero, names the missing default-branch ref, and publishes nothing

#### Scenario: A commit off the default branch, allowed

- **WHEN** HEAD is not an ancestor of `origin/main` and `verctl publish --allow-off-default-branch` runs
- **THEN** it publishes, and stderr says `allowed: off-default-branch` with the two commits

### Requirement: The workflows at the release commit match the default branch

`verctl publish` SHALL refuse, before any publish, tag, or Release call, when
the `.github/workflows/` tree at the release commit differs from the tree at
the default branch's tip. `--allow-workflow-change` SHALL let the run
continue.

#### Scenario: A retry after a workflow change

- **WHEN** a version commit is retried after `.github/workflows/release.yml` changed on `main`, and `verctl publish` runs at that commit
- **THEN** it exits non-zero, names `.github/workflows/`, and publishes nothing

#### Scenario: The same retry, allowed

- **WHEN** the same run passes `--allow-workflow-change`
- **THEN** it publishes, and the JSON report lists `workflow-change` under `allowed`

### Requirement: A package has one version commit since its last release

`verctl publish` SHALL refuse a package whose first-parent line from its
newest release tag to the release commit holds more than one commit that
changes its version. No flag SHALL allow it.

#### Scenario: A skipped version

- **WHEN** `demo` is tagged at 0.7.4, one first-parent commit sets it to 0.7.5, a later one sets 0.7.6, and `verctl publish` runs at the second
- **THEN** it exits non-zero, names 0.7.5 as unreleased, and publishes nothing

### Requirement: The workflow passes the allow flags through env

The reusable `release.yml` SHALL declare `allow-workflow-change` and
`allow-off-default-branch` as boolean inputs defaulting to `false`, and
SHALL pass them to verctl only through a step's `env:`.

#### Scenario: A caller allows a workflow change

- **WHEN** a caller passes `allow-workflow-change: true`
- **THEN** the publish step runs with `VERCTL_ALLOW_WORKFLOW_CHANGE=true` and no `run:` line contains the input

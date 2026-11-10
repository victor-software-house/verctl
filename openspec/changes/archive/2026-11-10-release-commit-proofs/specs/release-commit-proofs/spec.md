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
- **THEN** it publishes, and the report lists `off-default-branch` under `allowed` with the two commits

### Requirement: The workflows of the run commit match the default branch

`verctl publish` SHALL refuse when the `.github/workflows/` tree of the run
commit differs from the tree at the default branch's tip. The run commit is
the one `GITHUB_SHA` names when that is a commit in the repository, and HEAD
otherwise, because Actions runs the workflows of `GITHUB_SHA`. It SHALL
refuse before any publish, tag, or Release call. `--allow-workflow-change`
SHALL let the run continue.

#### Scenario: A retry after a workflow change

- **WHEN** a version commit is retried after `.github/workflows/release.yml` changed on `main`, and `verctl publish` runs at that commit
- **THEN** it exits non-zero, names `.github/workflows/`, and publishes nothing

#### Scenario: A workflow change merged after the Version PR

- **WHEN** HEAD is the version commit, `GITHUB_SHA` names the newer tip of `main`, and `.github/workflows/release.yml` changed between them
- **THEN** the run commit's workflows match the tip, and the proof passes

#### Scenario: The same retry, allowed

- **WHEN** the same run passes `--allow-workflow-change`
- **THEN** it publishes, and the JSON report lists `workflow-change` under `allowed`

### Requirement: A package has no skipped version since the last release

`verctl publish` SHALL refuse a package that has a skipped version in the
range from the release commit back to the newest release tag. The next
requirement defines a skipped version.

No flag SHALL allow it. The refusal SHALL name the way out: the unreleased
version, the commit that set it, and the command that publishes it first.

#### Scenario: A skipped version

- **WHEN** `demo` is tagged at 0.7.4. Commit `aaa1111` on the first-parent line sets it to 0.7.5, a later commit sets 0.7.6, and `verctl publish` runs at the later commit
- **THEN** it exits non-zero and publishes nothing
- **AND** the message names `demo` 0.7.5 as unreleased and `aaa1111` as the commit that set it
- **AND** the message names the next step: run `verctl publish --plan`, which lists `aaa1111` first, and publish that commit before this one

### Requirement: A skipped version has one definition

In the first-parent range from HEAD back to the stop point, version v is
skipped when all of these hold:

1. Some commit in the range has version v.
2. v is below HEAD's version.
3. No commit newer than v's oldest commit in the range has a version
   strictly below v, so v was not abandoned.

`verctl publish --plan` SHALL list every skipped version and no other.

#### Scenario: An abandoned version

- **WHEN** `demo` is tagged at 0.7.4. Commit `aaa1111` sets 0.7.5 and a later commit sets 0.7.6. The next commit reverts it to 0.7.5, and the release commit sets 0.7.7
- **THEN** 0.7.6 is abandoned, because a later commit took `demo` below it, and `verctl publish --plan` does not list it
- **AND** `verctl publish` at the release commit refuses only for 0.7.5 at `aaa1111`

#### Scenario: A version first set by a downward change

- **WHEN** `demo` is tagged at 0.7.4. Commit `aaa1111` sets 0.7.5 and a later commit sets 0.7.7. Commit `ccc3333` sets 0.7.6, and the release commit sets 0.7.8
- **THEN** `verctl publish --plan` lists 0.7.5 at `aaa1111` and 0.7.6 at `ccc3333`, and does not list 0.7.7

### Requirement: A skipped version is listed at the oldest commit

`verctl publish --plan` SHALL list each skipped v once, at the oldest commit
in the range whose version is v. A revert is then a later commit that
sets an older v again, and the listing stays at the original version commit.

#### Scenario: A version skipped before a revert

- **WHEN** `demo` is tagged at 0.7.4. Commit `aaa1111` sets 0.7.5 and a later commit sets 0.7.6. The next commit reverts it to 0.7.5, and the release commit sets 0.7.6 again
- **THEN** `verctl publish` at the release commit refuses, naming 0.7.5 and `aaa1111`, the commit that first set it, not the revert

### Requirement: The workflow passes the allow flags through env

The reusable `release.yml` SHALL declare `allow-workflow-change` and
`allow-off-default-branch` as boolean inputs defaulting to `false`, and
SHALL pass them to verctl only through a step's `env:`.

#### Scenario: A caller allows a workflow change

- **WHEN** a caller passes `allow-workflow-change: true`
- **THEN** the publish step runs with `VERCTL_ALLOW_WORKFLOW_CHANGE=true` and no `run:` line contains the input

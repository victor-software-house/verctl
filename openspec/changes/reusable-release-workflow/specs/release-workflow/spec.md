## ADDED Requirements

### Requirement: verctl ships one reusable release workflow

verctl SHALL ship `.github/workflows/release.yml` triggered only by
`workflow_call`. It SHALL declare the inputs `version-pr`, `publish`,
`assets`, `auto-merge`, `config`, `runner`, `mise-version`, `mise-env`,
`title`, `commit-message`, and `registry-token-env`, and the optional secrets
`app-client-id`, `app-private-key`, and `registry-token`, each with the
default the design's input table gives. A caller that passes no input SHALL
get verctl's defaults.

#### Scenario: A caller with no inputs

- **WHEN** a repository's `release.yml` calls `victor-software-house/verctl/.github/workflows/release.yml@<sha>` with only the App and registry secrets
- **THEN** the run plans, publishes, builds assets, and proposes, with config `.ctl/ver.yaml`, runner `ubuntu-latest`, `MISE_ENV=ci`, and title `chore(release): version packages`

#### Scenario: Renamed secrets

- **WHEN** a caller maps `registry-token: ${{ secrets.CRATES_TOKEN }}` and sets `registry-token-env: CARGO_REGISTRY_TOKEN`
- **THEN** the publisher runs with `CARGO_REGISTRY_TOKEN` set from `CRATES_TOKEN`

### Requirement: Each part can be turned off

Setting `version-pr`, `publish`, or `assets` to `false` SHALL skip the
matching job and SHALL NOT change what the other jobs do. `auto-merge`
SHALL default to `false`.

#### Scenario: A repository that publishes nothing

- **WHEN** a caller sets `publish: false` and a Version PR merge lands on `main`
- **THEN** the `publish` and `assets` jobs are skipped and `propose` still runs

#### Scenario: No assets

- **WHEN** `.ctl/ver.yaml` declares assets `darwin-arm64` and `linux-x64` and the caller sets `assets: false`
- **THEN** no asset job runs, and `verctl publish` creates the Release as it would for a package without assets

#### Scenario: No Version PR

- **WHEN** a caller sets `version-pr: false` and a fragment lands on `main`
- **THEN** no `propose` job runs and no `verctl/release/main` branch is pushed

#### Scenario: Auto-merge without the App

- **WHEN** a caller sets `auto-merge: true` and maps no `app-client-id`
- **THEN** the `propose` job fails before pushing, naming the App secrets

### Requirement: One flow on push to the default branch and on dispatch

The workflow SHALL run its jobs in the order `plan`, `publish`, `assets`,
`propose`. `publish` SHALL run only when `plan` reports a pending release, at
the version commit `plan` names. `propose` SHALL run only after `publish`
succeeded or was skipped. A dispatch SHALL run the same flow as a push. No
job SHALL read the pull request event, its head branch, or its labels.

#### Scenario: A Version PR merge

- **WHEN** a squash merge of `verctl/release/main` sets `demo` to `0.4.6` on `main`
- **THEN** `plan` reports that commit, `publish` releases `demo@0.4.6` there, and `propose` runs after it

#### Scenario: An ordinary push

- **WHEN** a commit that changes no manifest version lands on `main` and every release at its versions is complete
- **THEN** `plan` reports nothing pending, `publish` and `assets` are skipped, and `propose` opens or updates the Version PR

#### Scenario: A failed publish holds the next Version PR

- **WHEN** `cargo publish` fails in the `publish` job
- **THEN** `propose` does not run, and the next push to `main` plans the same version commit again

#### Scenario: A dispatch after a lost run

- **WHEN** the run for version commit `A` was replaced in its concurrency group before it started, and a person dispatches the workflow on `main` at a later commit `B`
- **THEN** `plan` reports `A`, and `publish` checks out `A` and releases it

### Requirement: Steps pass settings through the environment

Every workflow step that runs verctl SHALL set its settings in `env:` and run
`verctl <verb>` with no argument built in the shell.

#### Scenario: A config path

- **WHEN** a caller sets `config: release/ver.yaml`
- **THEN** each verctl step runs with `VERCTL_CONFIG=release/ver.yaml` and no `-c` argument

### Requirement: A repository migrates in one pull request

A repository SHALL be able to replace `version-pr.yml` and `publish.yml` with
one caller in one pull request. The migration guide SHALL say to close an open
`version-packages` pull request and delete that branch.

#### Scenario: Migrating with an open Version PR

- **WHEN** a repository merges its caller while a `version-packages` pull request is open, and closes it as the guide says
- **THEN** the next push to `main` opens a Version PR from `verctl/release/main` with the same pending fragments

## ADDED Requirements

### Requirement: verctl ships one reusable release workflow

verctl SHALL ship `.github/workflows/release.yml` triggered only by
`workflow_call`. It SHALL declare the inputs `version-pr`, `publish`,
`assets`, `docs`, `auto-merge`, `config`, `runner`, `mise-version`, `mise-env`,
`mise-ignored-config-paths`, `title`, `commit-message`, `registry-token-env`, and `crates-io-trusted-publishing`, and the optional secrets
`app-client-id`, `app-private-key`, and `registry-token`, each with the
default the design's input table gives. A caller that passes no input SHALL
get verctl's defaults.

#### Scenario: A caller with no inputs

- **WHEN** a repository's `release.yml` calls `victor-software-house/verctl/.github/workflows/release.yml@<sha>` with only the App and registry secrets
- **THEN** the run plans, publishes, builds assets, and proposes, with config `.ctl/ver.yaml`, runner `ubuntu-latest`, `MISE_ENV=ci`, and title `chore(release): version packages`

#### Scenario: Renamed secrets

- **WHEN** a caller maps `registry-token: ${{ secrets.CRATES_TOKEN }}` and sets `registry-token-env: CARGO_REGISTRY_TOKEN`
- **THEN** the publisher runs with `CARGO_REGISTRY_TOKEN` set from `CRATES_TOKEN`

#### Scenario: Trusted publishing

- **WHEN** a caller grants `id-token: write`, sets `crates-io-trusted-publishing: true`, and maps no `registry-token`
- **THEN** the publish job exchanges the run's OIDC token for a crates.io token and the publisher runs with `CARGO_REGISTRY_TOKEN` set from it
- **AND** a caller that also maps `registry-token` fails the publish job before publishing

### Requirement: Each part can be turned off

Setting `version-pr`, `publish`, `assets`, or `docs` to `false` SHALL skip
the matching job and SHALL NOT change what the other jobs do. `auto-merge`
SHALL default to `false`.

#### Scenario: A repository that publishes nothing

- **WHEN** a caller sets `publish: false` and a Version PR merge lands on `main`
- **THEN** the `publish` and `assets` jobs are skipped and `propose` still runs

#### Scenario: No assets

- **WHEN** `.ctl/ver.yaml` declares assets `darwin-arm64` and `linux-x64` and the caller sets `assets: false`
- **THEN** no asset job runs, `verctl publish` creates the Release as it would for a package without assets, and the next push reports `nothing to release`

#### Scenario: No Version PR

- **WHEN** a caller sets `version-pr: false` and a fragment lands on `main`
- **THEN** no `propose` job runs and no `verctl/release/main` branch is pushed

#### Scenario: Auto-merge without the App

- **WHEN** a caller sets `auto-merge: true` and maps no `app-client-id`
- **THEN** the `propose` job fails before pushing, naming the App secrets

### Requirement: A caller can keep repository-only mise configs out of the release jobs

When `mise-ignored-config-paths` is set, the `plan`, `publish`, `assets`,
and `docs` jobs SHALL set `MISE_IGNORED_CONFIG_PATHS` to each listed path
resolved against the repository root before mise installs any tool. The
`propose` job SHALL NOT ignore them. When the input is empty, no job SHALL
set the variable.

#### Scenario: A root config with a Python stack

- **WHEN** a caller sets `mise-ignored-config-paths: mise.toml` and its `mise.release.toml` declares only verctl
- **THEN** `plan`, `publish`, `assets`, and `docs` install only the tools `mise.release.toml` and the other configs declare, and `propose` installs the root `mise.toml` tools too

#### Scenario: No ignored paths

- **WHEN** a caller leaves `mise-ignored-config-paths` empty and its `.miserc.toml` ignores `tools/mise.toml`
- **THEN** no job sets `MISE_IGNORED_CONFIG_PATHS`, and mise still ignores `tools/mise.toml`

### Requirement: One flow on push to the default branch and on dispatch

The workflow SHALL run its jobs in the order `plan`, `publish`, `assets`,
`propose`. `publish` SHALL run only when `plan` reports a pending release,
once per pending version commit, oldest first, one at a time. `propose` SHALL run only after `publish`
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

#### Scenario: Two pending version commits

- **WHEN** `core@0.3.1` set at commit `A` is on crates.io without its tag, and `app@0.2.1` set at a later commit `C` is unpublished
- **THEN** one run's `publish` matrix releases `A`, then `C`, and the next push reports `nothing to release`

#### Scenario: A dispatch after a lost run

- **WHEN** the run for version commit `A` was replaced in its concurrency group before it started, and a person dispatches the workflow on `main` at a later commit `B`
- **THEN** `plan` reports `A`, and `publish` checks out `A` and releases it

### Requirement: The workflow runs only on the default branch

Every job SHALL run only when `github.ref` names the default branch. The
caller SHALL grant `contents: write` and `pull-requests: write`.

#### Scenario: A dispatch from a feature branch

- **WHEN** a person dispatches the caller's workflow on branch `fix/typo`
- **THEN** no job runs, no `verctl/release/fix/typo` branch is pushed, and nothing is published

#### Scenario: A repository without the App

- **WHEN** a caller maps no App secrets, the repository enables "Allow GitHub Actions to create and approve pull requests", and a fragment lands on `main`
- **THEN** `propose` opens the Version PR with `GITHUB_TOKEN` under the caller's `pull-requests: write`

#### Scenario: A repository without the App or the Actions setting

- **WHEN** a caller maps no App secrets and that repository setting is off
- **THEN** `propose` exits non-zero and its error names the setting

### Requirement: Steps pass settings through the environment

Every workflow step that runs verctl SHALL set its settings in `env:` and run
`mise run ver <verb>` with no argument built in the shell. The served `ver`
task execs the verctl version pinned in it. No job SHALL build verctl from
source, except that verctl's own release run builds it once per platform and
every job of that run reuses the build through `verctl-artifact`.

#### Scenario: verctl's own release

- **WHEN** a push to verctl's `main` starts `self-release.yml`
- **THEN** its `build` job builds verctl once on ubuntu-latest and once on macos-latest, and no job of `release.yml` compiles verctl

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

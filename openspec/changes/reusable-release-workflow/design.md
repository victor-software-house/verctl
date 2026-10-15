# Design

## Context

Today a repository runs two workflows. `version-pr.yml` runs on push to the
default branch and calls `verctl prepare --pr`. `publish.yml` runs when a pull
request closes, and publishes only when that pull request was merged from
`version-packages` into the default branch, or on a manual dispatch from the
default branch. `verctl publish` then runs each publisher, matches its stderr
for "already published", and creates the tag and the Release at HEAD.

[release-state-machine][rsm] redesigns all of this for a breaking minor and
is paused. This change takes the part that needs no breaking change.

## 1. The reusable workflow

`.github/workflows/release.yml` declares `on: workflow_call` and nothing else,
so its inputs always carry their defaults. A caller pins it by commit:

```yaml
name: Release
on:
  push:
    branches: [main]
  workflow_dispatch:
permissions:
  contents: write
  pull-requests: write
jobs:
  release:
    uses: victor-software-house/verctl/.github/workflows/release.yml@<sha> # v0.4.6
    secrets:
      app-client-id: ${{ secrets.CHANGESET_VERSION_APP_CLIENT_ID }}
      app-private-key: ${{ secrets.CHANGESET_VERSION_APP_PRIVATE_KEY }}
      registry-token: ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

The caller grants `contents: write` for tags and Releases and
`pull-requests: write` for the Version PR: a called workflow's
`GITHUB_TOKEN` cannot exceed what the caller grants, and `propose` falls
back to it when no App secret is mapped. That fallback also needs the
repository or organization setting "Allow GitHub Actions to create and
approve pull requests", which is off by default; without it GitHub refuses
the pull request, and `propose` fails naming the setting.

Every job runs only when `github.ref` is the default branch
(`format('refs/heads/{0}', github.event.repository.default_branch)`). A
dispatch from any other branch runs no job, as [#153][pr-153] made the
publish workflow do, so `propose` never derives `verctl/release/<feature>`
and `plan` never releases a commit outside the default branch.

### 1.1 Inputs

Every input is optional. The default is verctl's, and setting it is the
override.

| Input | Type | Default | What it changes |
|:--|:--|:--|:--|
| `version-pr` | boolean | `true` | `false` skips the `propose` job: no Version PR is opened or updated |
| `publish` | boolean | `true` | `false` skips the `publish` job: no registry upload, tag, or Release |
| `assets` | boolean | `true` | `false` skips the `assets` matrix, even when `.ctl/ver.yaml` declares assets, and leaves assets out of the completeness check (`VERCTL_SKIP_ASSETS`) |
| `auto-merge` | boolean | `false` | `true` enables auto-merge on the Version PR; needs the App secrets |
| `config` | string | `""` | path to the declarations file; empty means `.ctl/ver.yaml` |
| `runner` | string | `["ubuntu-latest"]` | JSON label list for the `plan`, `publish`, and `propose` jobs; asset jobs take their runners from `.ctl/ver.yaml` |
| `mise-version` | string | the version verctl's own CI runs | the mise that installs the caller's tools, including verctl |
| `mise-env` | string | `ci` | `MISE_ENV` for every job |
| `title` | string | `""` | Version PR title; empty means `chore(release): version packages` |
| `commit-message` | string | `""` | Version PR commit message; empty means the title |
| `registry-token-env` | string | `CARGO_REGISTRY_TOKEN` | the variable the `registry-token` secret is exported as for the publisher |

| Secret | Required | Without it |
|:--|:--|:--|
| `app-client-id`, `app-private-key` | no | `propose` uses `GITHUB_TOKEN`, so the Version PR's own CI does not start, and `auto-merge: true` fails before any write |
| `registry-token` | no | the publisher runs with whatever credential its own config finds |

A caller renames secrets through its `secrets:` mapping, so the secret names
in a repository are the caller's choice. GitHub fixes the names a called
workflow declares, which is why they are inputs to nothing else.

### 1.2 Jobs

1. `plan` checks out the pushed commit and runs `verctl publish --plan`,
   which writes to `$GITHUB_OUTPUT` `commits` (every incomplete version
   commit, oldest first, as a JSON list), `pending`, and `assets` (a matrix
   of each version commit and target whose asset its Release lacks).
2. `publish` runs when `inputs.publish` and `pending` are true, as a matrix
   over `commits` with `max-parallel: 1`, so one run releases every pending
   version commit in history order. Each leg checks out its commit and runs
   `verctl publish`. Concurrency group `verctl-publish`, never cancelled.
3. `assets` runs after `publish` when `inputs.assets` is true and the matrix
   is not empty, one job per commit and target, each at its commit.
4. `propose` runs after `publish` succeeds or is skipped, when
   `inputs.version-pr` is true. It checks out the pushed commit and runs
   `verctl prepare --pr`. Concurrency group `verctl-propose`, newest wins.

Every step sets its settings in `env:` and runs `verctl <verb>`. No step
builds argv in a shell. This needs ctl-core's environment-bound flags
(CTC-022): `VERCTL_CONFIG`, `VERCTL_AUTO_MERGE`, and the rest are read by
verctl itself.

The `plan` and `propose` jobs run on every push to the default branch. On a
push that changes no version, `plan` reports nothing pending, `publish` and
`assets` are skipped, and `propose` opens or updates the Version PR as
`version-pr.yml` does today.

## 2. Idempotent publish

### 2.1 The registry decides "published"

A publisher recipe gains `query`: an argv, with the same placeholders plus
`{name}` and `{version}`, that exits zero and prints the version when the
registry has it. The stock recipes:

| Publisher | `query` |
|:--|:--|
| `cargo` | `cargo info {name}@{version}` with `--registry {registry}` when one is set |
| `bun` | `bun pm view {name}@{version} version` |
| `none` | none; the package publishes to no registry |

verctl runs `query` first. When it prints the version, the package is
`already` and its publisher does not run. Otherwise verctl runs the publisher,
then `query` again with a bounded backoff (registry metadata can lag a
publish by a short while), and fails unless the registry lists the version
within that bound. The bound is a constant with its reason on it.

The recipe is data, so the exact commands are checked against each tool's
documentation when they are written, and the reference goes in the doc
comment. Override: a `publishers` entry or a package's own `query`. Disable:
`query: []`, which falls back to today's stderr match for that recipe.

Alternative: verctl's own registry clients, as
[release-state-machine][rsm] section 3.5 plans. Lost for this slice: each
registry protocol and its auth is a project, and the tool that publishes
already knows the registry, its config file, and its credentials.

### 2.2 The version commit

A package's version commit is the newest commit on the first-parent history
of the default branch where its manifest version became the version it has at
HEAD. `verctl publish --plan` finds, for each package, its version commit and
whether its release is complete: registry, tag, and Release, plus every
declared asset unless assets are off. The workflow passes `assets: false` as
`VERCTL_SKIP_ASSETS=true` (`--skip-assets`), so a repository that turns
assets off still reaches `nothing to release`. It reports every incomplete
version commit, oldest first, or nothing.

A run that released only the oldest commit would leave a newer one waiting,
and `propose` pushes `verctl/release/<base>`, not the default branch, so no
new run would start until an unrelated push. The `publish` matrix therefore
covers every pending commit in one run.

`verctl publish` refuses to run unless HEAD is a version commit of at least
one package whose release is incomplete. The tag it creates is at HEAD, which
is then that version commit. So a run on a later commit can no longer tag a
commit that was never published.

Alternative: dispatch with a `commit` input to resume. Lost: a missed run, for
example a pending run replaced in its concurrency group, would then need a
person to find the commit. The plan job finds it on the next push.

### 2.3 Nothing to release

When every package's release at HEAD's versions is complete, `verctl publish`
and `verctl publish --plan` report `nothing to release` and exit zero. The
JSON report carries `"outcome": "nothing-to-release"`, or `"released"` when
the run changed anything.

## 3. The Version PR branch

`prepare --pr` commits on `verctl/release/<base>`, where `<base>` is the
branch the Version PR targets. `check --versions` exempts any branch named
`verctl/release/<base>` locally; the `verctl:version` label exemption in CI
stays. The branch is derived, not configured.

Alternative: a configurable branch name. Lost: the name identifies the Version
PR, and [release-state-machine][rsm] section 2.3 already chose the derived
form.

## 4. Auto-merge in Rust

`prepare --pr --auto-merge` reads the base branch's rules, refuses when none
requires a status check, and enables auto-merge as a squash titled
`<title> (#<number>)` with the PR description as body, the behaviour of
`auto-merge.sh` today. The workflow passes the job's own token as
`VERCTL_JOB_TOKEN`, and verctl refuses auto-merge when the token it holds is
that one, because a merge made with it starts no workflow.

Alternative: keep `auto-merge.sh` and call it from the reusable workflow.
Lost: a called workflow cannot run a script from verctl's repository without a
second checkout of verctl at the pinned commit, and the action and the
workflow would carry two copies of the same rule.

## 5. Caller migration

1. Add the caller as `.github/workflows/release.yml` and delete
   `version-pr.yml` and `publish.yml`, in one pull request.
2. Close the open `version-packages` pull request, if any, and delete the
   branch. The first push after the merge opens `verctl/release/<base>`.
3. Move any setting the old workflows carried into inputs. A repository that
   publishes nothing sets `publish: false`. One with no App sets nothing,
   turns on "Allow GitHub Actions to create and approve pull requests" in
   the repository or organization settings, and accepts that its Version
   PR's CI needs a manual start.

The template renders the caller from copier answers, so a repository on the
template migrates with `copier update`.

## 6. Non-goals

1. Releases from a maintenance base other than the default branch.
2. Releasing an older version that a newer merged version superseded.
3. Command renames and `.ctl/ver.yaml` changes from
   [release-state-machine][rsm].

[rsm]: ../release-state-machine/design.md
[pr-153]: https://github.com/victor-software-house/verctl/pull/153

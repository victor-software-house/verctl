# Design

## Context

Today a repository runs two workflows.

1. `version-pr.yml` runs on push to the default branch and calls
   `verctl prepare --pr`.
2. `publish.yml` runs when a pull request closes.
   It publishes only on a merge from `version-packages` into the default
   branch, or on a manual dispatch from the default branch.

`verctl publish` then runs each publisher, matches its stderr for "already
published", and creates the tag and the Release at HEAD.

[release-state-machine][rsm] redesigns all of this for a breaking minor and
is paused. This change takes the part that needs no breaking change.

This change holds the reusable workflow and its callers. The rest of the
same slice is split into four changes of its own.

The registry check is
[registry-publish-check][rpc], and the release at a version commit is
[release-at-version-commit][ravc]. The docs.rs check is
[docs-build-check][docs], and the Version PR branch and auto-merge are
[derived-version-pr-branch][dvpb].

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
  id-token: write
jobs:
  release:
    uses: victor-software-house/verctl/.github/workflows/release.yml@<sha> # v0.6.0
    with:
      crates-io-trusted-publishing: true
    secrets:
      app-client-id: ${{ secrets.CHANGESET_VERSION_APP_CLIENT_ID }}
      app-private-key: ${{ secrets.CHANGESET_VERSION_APP_PRIVATE_KEY }}
```

The caller grants three permissions:

1. `contents: write` for tags and Releases.
2. `id-token: write` for crates.io trusted publishing.
3. `pull-requests: write` for the Version PR.

A called workflow's `GITHUB_TOKEN` cannot exceed what the caller grants.
`propose` falls back to that token when no App secret is mapped.
That fallback also needs the repository or organization setting "Allow GitHub
Actions to create and approve pull requests", which is off by default.
Without it GitHub refuses the pull request, and `propose` fails naming the
setting.

Every job runs only when `github.ref` is the default branch
(`format('refs/heads/{0}', github.event.repository.default_branch)`).

A dispatch from any other branch runs no job, as [#153][pr-153] made the
publish workflow do.
So `propose` never derives `verctl/release/<feature>`, and `plan` never
releases a commit outside the default branch.

### 1.1 Inputs

Every input is optional. The default is verctl's, and setting it is the
override.

| Input | Type | Default | What it changes |
|:--|:--|:--|:--|
| `version-pr` | boolean | `true` | `false` skips the `propose` job: no Version PR is opened or updated |
| `publish` | boolean | `true` | `false` skips the `publish` job: no registry upload, tag, or Release |
| `assets` | boolean | `true` | `false` skips the `assets` matrix, even when `.ctl/ver.yaml` declares assets, and leaves assets out of the completeness check (`VERCTL_SKIP_ASSETS`) |
| `docs` | boolean | `true` | `false` skips the `docs` matrix: no docs.rs build check |
| `auto-merge` | boolean | `false` | `true` enables auto-merge on the Version PR; needs the App secrets |
| `config` | string | `""` | path to the declarations file; empty means `.ctl/ver.yaml` |
| `runner` | string | `["ubuntu-latest"]` | JSON label list for the `plan`, `publish`, and `propose` jobs; asset jobs take their runners from `.ctl/ver.yaml` |
| `mise-version` | string | the version verctl's own CI runs | the mise that installs the caller's tools, including verctl |
| `mise-env` | string | `ci` | `MISE_ENV` for every job |
| `mise-ignored-config-paths` | string | `""` | colon-separated mise config paths, relative to the repository root, that `plan`, `publish`, `assets`, and `docs` ignore (section 1.4); empty ignores none |
| `verctl-artifact` | string | `""` | for verctl's own release. Each job downloads the build `<prefix>-<runner.os>-<runner.arch>` uploaded earlier in the calling run and puts it first on PATH. Empty runs the verctl the caller pins (section 1.3) |
| `title` | string | `""` | Version PR title; empty means `chore(release): version packages` |
| `commit-message` | string | `""` | Version PR commit message; empty means the title |
| `registry-token-env` | string | `CARGO_REGISTRY_TOKEN` | the variable verctl hands the registry token to the publisher under |
| `crates-io-trusted-publishing` | boolean | `false` | publish to crates.io with a trusted-publishing token from `rust-lang/crates-io-auth-action`, handed to the publisher as `registry-token-env`, in place of the `registry-token` secret (see the note after the tables) |

| Secret | Required | Without it |
|:--|:--|:--|
| `app-client-id`, `app-private-key` | no | `propose` uses `GITHUB_TOKEN`, so the Version PR's own CI does not start, and `auto-merge: true` fails before any write |
| `registry-token` | no | the publisher runs with whatever credential its own config finds |

A caller renames secrets through its `secrets:` mapping, so the secret names
in a repository are the caller's choice. GitHub fixes the names a called
workflow declares, which is why they are inputs to nothing else.

With `crates-io-trusted-publishing`, the token comes from
`rust-lang/crates-io-auth-action` and is exported as `registry-token-env`.
The publish job requests `id-token: write`, which every caller grants.

### 1.2 Jobs

1. `plan` checks out the pushed commit and runs `verctl publish --plan`
   (`VERCTL_PLAN=true`).
   It writes three outputs to `$GITHUB_OUTPUT`:
   - `commits`: every incomplete version commit, oldest first, as a JSON list.
   - `pending`.
   - `assets`: a matrix of each version commit and target whose asset its
     Release lacks.
2. `publish` runs when `inputs.publish` and `pending` are true.
   - It runs as a matrix over `commits` with `max-parallel: 1` and
     `fail-fast: true`.
   - One run therefore releases every pending version commit in history order
     and stops at the first that fails.
   - Each leg checks out its commit and runs `verctl publish`.
   - Its concurrency group is `verctl-publish` with
     `cancel-in-progress: false`, so a running leg is never cancelled.
   - GitHub keeps one pending job per group, so a leg still waiting when a
     newer run's leg arrives is cancelled.
   - The newer run planned every incomplete commit itself, so it publishes
     that commit, and the older run's `propose` is skipped.
   - Workflow-level concurrency would not avoid this (it keeps one pending
     run the same way) and would make `plan` and `propose` wait too.
3. `assets` runs after `publish` when `inputs.assets` is true and the matrix
   is not empty, one job per commit and target, each at its commit.
4. `docs` runs after `publish` finishes, failed or not (`!cancelled()`), when
   `inputs.docs` and `pending` are true.
   - It runs as a matrix over `commits` with no parallel limit, each leg at its
     commit running `verctl docs` ([docs-build-check][docs]).
   - A commit released before a later leg failed is still checked.
   - A commit whose release tag is missing has nothing to check.
   - It never holds `propose` back.
5. `propose` runs after `publish` succeeds or is skipped, when
   `inputs.version-pr` is true. It checks out the pushed commit and runs
   `verctl prepare --pr` (`VERCTL_PR=true`). Concurrency group
   `verctl-propose`, newest wins.

Every step sets its settings in `env:` and runs `mise run ver <verb>`, the
served task, with the verctl version pinned in that task.
So a caller's pinned verctl runs.
In verctl's own repository each job runs the build section 1.3 describes.
No step builds argv in a shell.

Flags such as `--plan` and `--pr` are set as `VERCTL_PLAN` and `VERCTL_PR`.
This needs ctl-core's environment-bound flags (queue row
[CTC-022][ctc-022]): `VERCTL_CONFIG`, `VERCTL_AUTO_MERGE`, and the rest are
read by verctl itself.

The `plan` and `propose` jobs run on every push to the default branch. On a
push that changes no version, `plan` reports nothing pending, `publish` and
`assets` are skipped, and `propose` opens or updates the Version PR as
`version-pr.yml` does today.

### 1.3 Building verctl

verctl is built from source only for a release, and only in CI, never in
every job. If a pull-request or merge workflow already built the binary, the
release reuses that artifact rather than building again. In verctl's own
repository the release run builds once (or reuses), and every other job uses
the pinned verctl.

`self-release.yml` does this with a `build` job ahead of the reusable
workflow.
The job runs one `cargo build --release` per platform the release runs on:

- ubuntu-latest for `plan`, `publish`, `docs`, `propose`, and the `linux-x64`
  asset.
- macos-latest for the `darwin-arm64` asset.

Each build is uploaded as `verctl-<runner.os>-<runner.arch>`.

`self-release.yml` calls `release.yml` with `verctl-artifact: verctl` and
`mise-env: ci,release`.
Each job downloads the build for its own platform and puts it first on PATH.
`mise.release.toml`'s `ver` task runs `verctl` from PATH with no pin, so a
pinned tool cannot shadow it.

The release needs the verctl it releases and cannot use a pin.
At a version commit the served `tasks/ver/ver` already names the new version,
whose tarball does not exist until `publish` uploads it.
The previous release cannot read the `VERCTL_*` settings the steps set.

A job runs on one machine, so a Linux build cannot serve the macOS asset job.
That is why there is one build per platform.

Reusing a binary from the merge's CI run waits until CI uploads one.
Until then the release builds its own.
A consumer passes no `verctl-artifact` and always runs its pin.

verctl's CI `plan` job still runs `cargo run` for `verctl ci`.
Moving it to a pinned verctl needs a pin outside the served task, which the
Version PR rewrites to a version not yet published.
That move is left to a follow-up.

### 1.4 Ignored mise config paths

A repository's root `mise.toml` can declare tools only its own tasks need,
such as a Python stack with a `uv sync` postinstall.
The release jobs run only verctl and the publisher.
Installing that stack in each of them costs minutes and can fail on tools the
release never starts.

`mise-ignored-config-paths` names those files.
The `plan` job checks each entry once and exposes the list as an output.
In `plan`, `publish`, `assets`, and `docs`, a one-line step before the mise
action writes the list to `MISE_IGNORED_CONFIG_PATHS` in `GITHUB_ENV`.
So the action and every later step skip them.
The entries stay relative.
mise resolves a relative entry against the directory it runs in since 2026.8.9
(jdx/mise#12169), and every step runs at the repository root.
The input defaults to mise 2026.10.25, and every caller passed 2026.9.17 or
later on 2026-11-10.

Joining each entry onto `GITHUB_WORKSPACE` in `plan` was considered and
rejected, because an asset runner on another operating system has another
workspace path.
A job-level `env:` was also rejected.
GitHub sets a job variable even when its value is empty, which would replace
the paths `.miserc.toml` ignores.

Each entry must be a plain relative path.
An entry fails the `plan` job when it has an empty, `.`, or `..` segment, or a
leading or trailing slash.
Such an entry could name the repository root or a path outside it.

The jobs that ignore files install fewer tools than `propose`, so they take
the mise-action cache prefix `mise-v1-trimmed` and never share its cache.

The step runs only when the input is set. mise reads the variable whenever
it is present, so an empty value would replace the ignored paths a
repository's `.miserc.toml` declares.

`propose` keeps every config: `prepare.after` runs the repository's own
tasks, such as a version sync, which may need those tools. An asset build
that needs a toolchain from an ignored file names it in its own task, for
example `mise -C cli x -- cargo build`.

## 2. Caller migration

1. Add the caller as `.github/workflows/release.yml` and delete
   `version-pr.yml` and `publish.yml`, in one pull request.
2. Close the open `version-packages` pull request, if any, and delete the
   branch. The first push after the merge opens `verctl/release/<base>`.
3. Move any setting the old workflows carried into inputs.
   - A repository that publishes nothing sets `publish: false`.
   - One with no App sets nothing.
     It turns on "Allow GitHub Actions to create and approve pull requests" in
     the repository or organization settings.
     It accepts that its Version PR's CI needs a manual start.

The template renders the caller from copier answers, so a repository on the
template migrates with `copier update`.

## 3. Semver

The change ships in a patch release with no major bump, by the operator's
ruling on 2026-10-17.
Three of its changes look breaking:

1. Its new flags add fields to the clap argument structs in `cli`.
2. The `docs` command adds a `cli::Command` variant.
3. The derived Version PR branch removes `release::VERSION_BRANCH`.

cargo-semver-checks reports each of those as breaking, though only verctl's
own binary builds the `cli` types.
The per-lint table in `Cargo.toml` that queue row [VER-065][queue] removes
after the release therefore warns on `constructible_struct_adds_field`,
`struct_marked_non_exhaustive`, `pub_module_level_const_missing`, and
`enum_variant_added`, beside the existing `function_parameter_count_changed`.

The `cli` argument structs become `#[non_exhaustive]` in this release, so a
later flag is never a breaking change again.
`cli::Command` stays exhaustive, because the binary matches it without a
wildcard arm.

## 4. Non-goals

1. Releases from a maintenance base other than the default branch.
2. Releasing an older version that a newer merged version superseded.
3. Command renames and `.ctl/ver.yaml` changes from
   [release-state-machine][rsm].

[queue]: ../../../tasks.yaml
[ctc-022]: https://github.com/victor-software-house/ctl-core/blob/main/tasks.yaml
[rsm]: ../release-state-machine/design.md
[pr-153]: https://github.com/victor-software-house/verctl/pull/153
[docs]: ../docs-build-check/design.md
[rpc]: ../registry-publish-check/design.md
[ravc]: ../release-at-version-commit/design.md
[dvpb]: ../derived-version-pr-branch/design.md

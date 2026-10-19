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
| `docs` | boolean | `true` | `false` skips the `docs` matrix: no docs.rs build check |
| `auto-merge` | boolean | `false` | `true` enables auto-merge on the Version PR; needs the App secrets |
| `config` | string | `""` | path to the declarations file; empty means `.ctl/ver.yaml` |
| `runner` | string | `["ubuntu-latest"]` | JSON label list for the `plan`, `publish`, and `propose` jobs; asset jobs take their runners from `.ctl/ver.yaml` |
| `mise-version` | string | the version verctl's own CI runs | the mise that installs the caller's tools, including verctl |
| `mise-env` | string | `ci` | `MISE_ENV` for every job |
| `verctl-artifact` | string | `""` | for verctl's own release: each job downloads the build `<prefix>-<runner.os>-<runner.arch>` uploaded earlier in the calling run and puts it first on PATH; empty runs the verctl the caller pins (section 1.3) |
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

1. `plan` checks out the pushed commit and runs `verctl publish --plan`
   (`VERCTL_PLAN=true`),
   which writes to `$GITHUB_OUTPUT` `commits` (every incomplete version
   commit, oldest first, as a JSON list), `pending`, and `assets` (a matrix
   of each version commit and target whose asset its Release lacks).
2. `publish` runs when `inputs.publish` and `pending` are true, as a matrix
   over `commits` with `max-parallel: 1` and `fail-fast: true`, so one run
   releases every pending version commit in history order and stops at the
   first that fails. Each leg checks out its commit and runs
   `verctl publish`. Concurrency group `verctl-publish` with
   `cancel-in-progress: false`, so a running leg is never cancelled. GitHub
   keeps one pending job per group, so a leg still waiting when a newer
   run's leg arrives is cancelled; the newer run planned every incomplete
   commit itself, so it publishes that commit, and the older run's `propose`
   is skipped. Workflow-level concurrency would not avoid this (it keeps one
   pending run the same way) and would make `plan` and `propose` wait too.
3. `assets` runs after `publish` when `inputs.assets` is true and the matrix
   is not empty, one job per commit and target, each at its commit.
4. `docs` runs after `publish` finishes, failed or not (`!cancelled()`), when
   `inputs.docs` and `pending` are true, as a matrix over `commits` with no
   parallel limit, each leg at its commit running `verctl docs`
   (section 2.4). A commit released before a later leg failed is still
   checked; a commit whose release tag is missing has nothing to check. It
   never holds `propose` back.
5. `propose` runs after `publish` succeeds or is skipped, when
   `inputs.version-pr` is true. It checks out the pushed commit and runs
   `verctl prepare --pr` (`VERCTL_PR=true`). Concurrency group
   `verctl-propose`, newest wins.

Every step sets its settings in `env:` and runs `mise run ver <verb>`, the
served task, with the verctl version pinned in that task, so a caller's pinned
verctl runs. In verctl's own repository each job runs the build section 1.3
describes. No step builds argv in a shell.
Flags such as `--plan` and `--pr` are set as `VERCTL_PLAN` and `VERCTL_PR`.
This needs ctl-core's environment-bound flags
(CTC-022): `VERCTL_CONFIG`, `VERCTL_AUTO_MERGE`, and the rest are read by
verctl itself.

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
workflow: one `cargo build --release` per platform the release runs on
(ubuntu-latest for `plan`, `publish`, `docs`, `propose`, and the `linux-x64`
asset; macos-latest for the `darwin-arm64` asset), each uploaded as
`verctl-<runner.os>-<runner.arch>`. It calls `release.yml` with
`verctl-artifact: verctl` and `mise-env: ci,release`. Each job downloads the
build for its own platform and puts it first on PATH, and
`mise.release.toml`'s `ver` task runs `verctl` from PATH with no pin, so a
pinned tool cannot shadow it.

The release needs the verctl it releases, not a pin: at a version commit the
served `tasks/ver/ver` already names the new version, whose tarball does not
exist until `publish` uploads it, and the previous release cannot read the
`VERCTL_*` settings the steps set. A job runs on one machine, so a Linux build
cannot serve the macOS asset job; that is why there is one build per platform
rather than one in all.

Reusing a binary from the merge's CI run waits until CI uploads one; until
then the release builds its own. A consumer passes no `verctl-artifact` and
always runs its pin. verctl's CI `plan` job still runs `cargo run` for
`verctl ci`; moving it to a pinned verctl needs a pin outside the served
task, which the Version PR rewrites to a version not yet published, and is
left to a follow-up.

## 2. Idempotent publish

### 2.1 The registry decides "published"

A publisher recipe gains `query`: an argv, with the same placeholders plus
`{name}` and `{version}`, that exits zero and prints the version when the
registry has it. It runs in the package's directory, as the publisher does,
so the registry config the publisher reads (`bunfig.toml`, `.npmrc`) is in
effect. The stock recipes:

| Publisher | How "published" is decided |
|:--|:--|
| `cargo` whose publish target is crates.io (below) | verctl reads the crate's file in the crates.io sparse index, `https://index.crates.io/<prefix>/<name>`, with the prefix and the file name built from the lowercased crate name, and looks for a line whose `vers` equals the version as a semver version |
| `cargo` with any other target | the package's own `query`, or the stderr match when it declares none |
| `bun` | `query`: `bun pm view {name}@{version} version` |
| `none` | nothing; the package publishes to no registry |

A cargo package's publish target is crates.io only when every place Cargo
reads it agrees: the package sets no `registry` in `.ctl/ver.yaml` (or sets
`crates-io`), its manifest's `publish` field is absent or names only
`crates-io`, and the `registry.default` Cargo resolves for the publisher's
working directory is unset or `crates-io`. verctl resolves it in Cargo's
documented order ([config hierarchy][cargo-config]): `CARGO_REGISTRY_DEFAULT`,
then each `.cargo/config.toml` (or legacy `.cargo/config`) from the directory
the publisher runs in up to the filesystem root, then
`$CARGO_HOME/config.toml` (or legacy `$CARGO_HOME/config`), with
`CARGO_HOME` defaulting to `~/.cargo`, the first that sets it winning. The
stock cargo publisher runs from the repository root with `--manifest-path`,
so for it the walk starts there, not in the package's directory. When a
repository overrides the cargo publisher, verctl cannot know the directory it
runs in, so the target is unknown and the package needs `query`. When a file it reads cannot be parsed or uses
`include`, the target is unknown. `cargo config get` would answer directly
but needs a nightly Cargo, which a consumer's release job need not have.
Any target other than crates.io, and an unknown one, needs `query` for a
registry check. The same holds for an overridden upload command of any
publisher: a bun package whose own argv runs `npm publish --registry …` would
otherwise be checked against the registry the stock `bun pm view` asks.

For a package with a registry check (the crates.io read or a `query`),
verctl decides first. When the registry has the version, the package is
`already` and its publisher does not run. Otherwise verctl runs the publisher,
then decides again with a bounded backoff (registry metadata can lag a
publish by a short while), and fails unless the registry lists the version
within that bound. A check that fails during that wait (an index timeout, a
5xx) counts as not listed for that round, because the upload already
happened; the last round's error is reported if the bound runs out. The bound
is a constant with its reason on it. A package
with no registry check keeps today's rule: the publish succeeds on a zero
exit, or on stderr that says the version already exists.

The crates.io check is a direct index read because `cargo info`, run in a
Cargo workspace, can answer for a workspace member from its local manifest
and report an unpublished crate as `already`. Running it outside the
workspace lost: Cargo then loses the checkout's registry config, and copying
that config into a temporary directory drags in parent-directory configs,
`include` targets, and relative paths. The sparse index is public, needs no
auth, and is Cargo's own source of truth for crates.io
([sparse index][cargo-sparse]). Every repository in scope publishes crates to
crates.io; another registry declares `query`.

The recipe is data, so the exact commands are checked against each tool's
documentation when they are written, and the reference goes in the doc
comment. Override: a `publishers` entry or a package's own `query`, which
also replaces the crates.io read. Disable: `query: []`, which falls back to
today's stderr match for that package.

The index read and the docs.rs check (section 2.4) use `reqwest`, declared
directly as 0.13.5 with `default-features = false` and the `rustls` feature.
The operator approved the dependency on 2026-10-17. gix's HTTP transport
already builds that version with rustls, so no crate enters the build.
octocrab is no substitute: pointed at another host it would send the GitHub
token there. Starting `curl` would make it a tool every consumer must
declare.

Alternative: verctl's own clients for every registry, as
[release-state-machine][rsm] section 3.5 plans. Lost for this slice: each
authenticated registry protocol is a project, while the crates.io index read
is one unauthenticated request.

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
one package whose release is incomplete, and it publishes, tags, and
releases only the packages whose version commit is HEAD. Every other
incomplete package is left for the run at its own version commit, and the
report names it. The tag it creates is at HEAD, which is then that
package's version commit. So a run on a later commit can no longer tag a
commit that was never published.

A tag that already exists names the release commit when it sits on another
first-parent commit where the package has the same version. A publish rerun
after later commits tagged a newer HEAD that way, and a version set again
after a revert keeps the tag from its first release; in both cases the
registry already has the version, so that commit is the release and its
assets build there. A tag on a commit with another version, or off the
first-parent history, fails the plan: that release was tagged in the wrong
place, and counting it complete would hide it.

Alternative: dispatch with a `commit` input to resume. Lost: a missed run, for
example a pending run replaced in its concurrency group, would then need a
person to find the commit. The plan job finds it on the next push.

### 2.3 Nothing to release

When every package's release at HEAD's versions is complete, `verctl publish`
and `verctl publish --plan` report `nothing to release` and exit zero. The
JSON report carries `"outcome": "nothing-to-release"`, or `"released"` when
the run changed anything.

### 2.4 The docs.rs build

`verctl docs` takes the packages released at HEAD whose publish target is
crates.io (section 2.1) and that have a library target as Cargo resolves it:
an explicit `[lib]` table, or `src/lib.rs` found by target auto-discovery
unless `package.autolib` is `false`. docs.rs documents no binary-only crate.
verctl applies Cargo's documented rule itself rather than running
`cargo metadata`, so the `docs` job needs no Cargo. It checks each such
package only when the tag `tags.template` names for that package's version
at HEAD names HEAD on GitHub: the shared `v{version}` tag, or the package's
own tag when the template has `{name}`. verctl reads the tag through the
GitHub API, peeling an annotated tag to its commit, never from local refs:
the `docs` job's checkout fetched no tags, the `publish` job pushed the tag
from another runner, and the API needs no git credentials in the checkout.
`verctl publish`
creates tags only after the registry lists every version (section 2.1), so
the tag is the record that the publish happened, and no read of a lagging
index decides it. A crate that reached crates.io while another package
failed has no tag yet; its commit stays incomplete, and the rerun that
completes it tags the crate and checks it then. It polls
`https://docs.rs/crate/<name>/<version>/status.json` with a bounded backoff.
docs.rs answers 404 until its build of the version starts, then
`{"doc_status": <bool>, "version": "<version>"}`. `doc_status` is false while
the build runs and after it failed; the endpoint does not tell the two apart.
verctl polls until every crate reports `doc_status` true or `--timeout`
(minutes, default 60, `VERCTL_TIMEOUT`) runs out. It then reports each crate
as `built`, `failed` (still false), or `unbuilt` (still 404), and exits
non-zero naming every crate that is not built. A failed build therefore fails
the job at the timeout, not when docs.rs records it. docs.rs can queue a
build for a long time, so the check is its own job: a slow queue never
delays the next version commit's publish or the Version PR.

The docs.rs build does not count toward completeness (section 2.3). A failed
build cannot be retried for the same version, so a rerun reports `nothing to
release` and the fix is the next patch release. The failed `docs` job is the
visible record.

## 3. The Version PR branch

`prepare --pr` commits on `verctl/release/<base>`, where `<base>` is the
branch the Version PR targets. `check --versions` exempts any branch named
`verctl/release/<base>` locally, and in CI a pull request with that head
branch, this repository as its head repository, and `<base>` as its current
base ([release-state-machine][rsm] section 2.1). The branch is derived, not
configured.

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

## 6. Semver

The change ships in a patch release with no major bump, by the operator's
ruling on 2026-10-17. Its new flags add fields to the clap argument structs
in `cli`, the `docs` command adds a `cli::Command` variant, and the derived
Version PR branch removes `release::VERSION_BRANCH`. cargo-semver-checks calls
each of those breaking, though only verctl's own binary builds the `cli`
types. The per-lint table in `Cargo.toml` that VER-065 removes after the
release therefore warns on `constructible_struct_adds_field`,
`struct_marked_non_exhaustive`, `pub_module_level_const_missing`, and
`enum_variant_added`, beside the existing `function_parameter_count_changed`.
The `cli` argument structs become `#[non_exhaustive]` in this release, so a
later flag is never a breaking change again. `cli::Command` stays exhaustive,
because the binary matches it without a wildcard arm.

## 7. Non-goals

1. Releases from a maintenance base other than the default branch.
2. Releasing an older version that a newer merged version superseded.
3. Command renames and `.ctl/ver.yaml` changes from
   [release-state-machine][rsm].

[rsm]: ../release-state-machine/design.md
[pr-153]: https://github.com/victor-software-house/verctl/pull/153
[cargo-sparse]: https://doc.rust-lang.org/cargo/reference/registry-index.html#sparse-protocol
[cargo-config]: https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure

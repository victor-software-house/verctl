# Design

## Context

See [proposal.md](proposal.md) for why, and [`release-asset-prebuilt`](specs/release-asset-prebuilt/spec.md) for what.

`self-release.yml` builds verctl once per runner OS and architecture and uploads `verctl-<runner.os>-<runner.arch>`.
Every `release.yml` job downloads the build of its own runner into `${{ runner.temp }}/verctl-build` and puts it first on `PATH`.
The `assets` job then runs `verctl release asset`, whose `build` compiles the target again.

## Decisions

### 1. `--prebuilt <path>` on `release asset`

`assets::build` takes the optional path.
With it, `prepare` and `build` are skipped, the file is copied as `{bin}` with mode `0755` into a fresh directory, and `tar` cuts the tarball from there.
The tarball name and layout are the ones a build gives: one entry named `{bin}`.

1. The target must pack the one binary.
   A target or recipe that packs `files` under `dir` fails, naming the target, because a prebuilt binary replaces only the one-binary pack.
2. The path must be a file.
3. `--prebuilt` conflicts with `--vendored`, whose archive packs more than one file.
4. The report gains a `prebuilt` field, the path, only when the flag is set, so the existing JSON stays the same.
5. The fresh directory is `.verctl-prebuilt-{id}` under the repository root, removed once `tar` ends.
   `tempfile` is a development dependency, so a system temp directory would need a new runtime dependency.

Alternatives considered:

1. **Point `assets.binary` at the downloaded file and set `assets.build` to a no-op in `.ctl/ver.yaml`.**
   It lost because it changes the declared recipe for every version commit, so an older commit would pack this run's binary under its own tag.
2. **Upload the tarball from the workflow with `gh release upload`.**
   It lost because `release asset` owns the tarball name, the draft lookup, and the skip of an asset already uploaded.
3. **Reuse the binary inside `build` when it finds one on `PATH`.**
   It lost because it is a hidden behaviour a consumer cannot see or turn off.

Public override and disable path: omit `--prebuilt` and `VERCTL_PREBUILT`, and `release asset` builds as before.
An empty `VERCTL_PREBUILT` is unset by ctl-core's rule for every `VERCTL_*` variable.

### 2. When the workflow reuses the build

The `Build and upload` step sets `VERCTL_PREBUILT` to `${{ runner.temp }}/verctl-build/verctl` only when `inputs.verctl-artifact` is set and `matrix.commit == github.sha`.
The build job compiled `github.sha`, so the binary is the version commit's only when the two match.
An older pending version commit keeps compiling its own tree, which is also what its tag names.

Each platform's asset runs on the runner whose OS and architecture the build job used.
The `linux-x64` target's triple is the host triple of `ubuntu-26.04`, and `linux-arm64`'s is the host triple of `ubuntu-26.04-arm`, so the host build is the target build.
`darwin-arm64` is `macos-latest`, which is arm64.

Alternatives considered:

1. **Always pack the run's build.**
   It lost because an older version commit would ship a binary built from newer source.
2. **Compare the version inside the binary with the plan's version.**
   It lost because the commit comparison is exact and runs before any step starts.

Public override and disable path: a caller that omits `verctl-artifact` never sets the variable, so the mbx-cached compile still runs.
The mbx step stays for those runs and for older commits.

### 3. `--locked` in `mise run build`

The stock asset build passes `--locked`, and the reused binary is the `mise run build` output.
Without `--locked` a stale `Cargo.lock` would build and ship a binary the lock does not describe.

### 4. Ubuntu 26.04 defaults

Every default GitHub-hosted Ubuntu label verctl owns changes to `ubuntu-26.04` or `ubuntu-26.04-arm`.
`nscloud-ubuntu-24.04-*` Namespace labels are not GitHub-hosted defaults and stay.
CHANGELOG entries and archived changes keep the labels they recorded.

A Linux tarball built on the defaults links the glibc of Ubuntu 26.04.
The changeset says so.

Alternative considered: keep `ubuntu-latest`, which GitHub moves on its own schedule.
It lost because the default would change the glibc of the tarballs without a commit in this repository.

Public override and disable path: a caller sets the `runner` input of `release.yml`, `ci.<job>.runners`, or `runners.<name>.labels`, and a built-in target takes `runner: <name>`.

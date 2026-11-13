# One verctl build per platform in the self-release

## Motivation

verctl's own release compiles the same verctl twice on each platform.
The `build` job of `self-release.yml` compiles it so every `release.yml` job can run it.
Then each `assets` job runs `verctl release asset`, which runs the stock `cargo build --release --locked --target {triple}` again before it packs the tarball.
Each platform therefore pays for a second compile of the same commit, in a job that has the first binary on disk.

The default GitHub-hosted Ubuntu machines are also `ubuntu-latest` and `ubuntu-24.04-arm`.
`ubuntu-26.04` and `ubuntu-26.04-arm` are the images this repository's template already uses for its own jobs.
verctl owns the defaults for its reusable workflow, its built-in `linux-x64` target, and `ci`, and they should name the same image.

Queue row: [VER-128][ver-128].

## Changes

1. `verctl release asset` takes `--prebuilt <path>` (`VERCTL_PREBUILT`).
   With it, neither `prepare` nor `build` runs, and the file is packed as `{bin}`.
   It conflicts with `--vendored`, and a target that packs `files` under `dir` fails naming the target.
2. In `release.yml`, an `assets` job sets `VERCTL_PREBUILT` to the downloaded `verctl-artifact` build when the version commit is the run's commit.
   An older version commit, and a run without `verctl-artifact`, compile as before.
3. `mise run build` passes `--locked`, so the reused binary is built from the lock as the stock asset build is.
4. Every default GitHub-hosted Ubuntu label verctl owns becomes `ubuntu-26.04` (x64) or `ubuntu-26.04-arm` (arm64): the `runner` input of `release.yml`, the built-in `linux-x64` target, the `ci` default, `self-release.yml`, `ci.yml`, `.ctl/ver.yaml`, the examples, and the docs.

## Capabilities

### New Capabilities

- `release-asset-prebuilt`: `release asset` packs a binary built earlier, and the default Ubuntu machines are 26.04.

### Modified Capabilities

None.

## Impact

1. Code: `src/cli.rs`, `src/assets.rs`, `src/main.rs`, `src/presentation.rs`, and the label strings in `src/ci.rs`, `src/runners.rs`, and `src/config.rs`.
2. Workflows: `release.yml`, `self-release.yml`, `ci.yml`.
3. Docs: the operator instructions and skill with their templates, the README, `AGENTS.md`, and the examples.
4. Release: a patch.
   A Linux tarball built on the default machines links the glibc of Ubuntu 26.04, so it needs that glibc or newer.

[ver-128]: ../../../tasks.yaml

# Tasks

The pull request's CI verifies every compile, format, lint, and test task.

## 1. Prebuilt binary

- [ ] 1.1 `release asset` takes `--prebuilt <path>` (`VERCTL_PREBUILT`), skips `prepare` and `build`, and packs the file as `{bin}`.
  Verify: the `assets` prebuilt unit tests pass in CI.
- [ ] 1.2 A target that packs `files`, a path that is not a file, and `--vendored` fail.
  Verify: the `assets` prebuilt unit tests and the `cli` `release_asset_takes_a_prebuilt_binary` test pass in CI.
- [ ] 1.3 The `assets` job sets `VERCTL_PREBUILT` only when `verctl-artifact` is set and the version commit is the run's commit.
  Verify: `mise run lint:workflows` and `mise run lint:workflow-policy` pass in CI.
- [ ] 1.4 `mise run build` passes `--locked`.
  Verify: `rg -- '--locked' mise-tasks/build.toml` finds the task.

## 2. Ubuntu 26.04

- [ ] 2.1 No default GitHub-hosted label outside the archive, the changelog, and Namespace labels names `ubuntu-latest` or `ubuntu-24.04`.
  Verify: `rg 'ubuntu-latest|ubuntu-24' --glob '!openspec/changes/archive/**' --glob '!CHANGELOG.md'` finds only `nscloud-` labels and closed designs.
- [ ] 2.2 The operator instructions, the skill, the README, and `AGENTS.md` describe `--prebuilt` and the 26.04 defaults.
  Verify: `cargo test operator_docs` passes in CI.

## 3. Proof

- [ ] 3.1 The next verctl release packs each platform's tarball from the `build` job's binary.
  Verify: the release run's link, recorded here, shows no `cargo build` in an `assets` leg.
- [ ] 3.2 A `.changeset/` patch fragment.
  Verify: `verctl check` in CI passes on the pull request.

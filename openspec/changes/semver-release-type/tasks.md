# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Baseline

- [ ] 1.1 `crates_io_baseline` picks the newest stable, unyanked release at or below the version; proof: the baseline tests in `src/registry.rs` pass in CI

## 2. Decision and run

- [ ] 2.1 `skip_reason` skips a minor fragment, a moved minor, and an unpublished crate, and fails at 1.0; proof: the tests in `src/semver_check.rs` pass in CI
- [ ] 2.2 `verctl semver` runs one check per remaining crate and fails naming each break; proof: code review of `semver_check::run`
- [ ] 2.3 Running `verctl semver` in a consumer at 0.0.z reports its crates checked at `--release-type minor`; proof: a run on a released consumer after this ships

## 3. Docs

- [ ] 3.1 The operator instructions and the skill describe the verb; proof: `cargo test operator_docs` passes in CI

## 4. ctl-core

- [ ] 4.1 ctl-core's shared `semver` task runs `mise run ver semver`, and ctl-core and qctl serve `ver`; proof: a ctl-core pull request after this release, with CI green

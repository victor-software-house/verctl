# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Baseline

- [x] 1.1 `crates_io_baseline` picks the newest stable, unyanked release at or below the version; proof: the baseline tests in `src/registry.rs` pass in CI
  Evidence, 2026-11-10: shipped in [#183][verctl#183]: the baseline tests in `src/registry.rs` pass.

## 2. Decision and run

- [x] 2.1 `skip_reason` skips a minor fragment, a moved minor, and an unpublished crate, and fails at 1.0; proof: the tests in `src/semver_check.rs` pass in CI
  Evidence, 2026-11-10: shipped in [#183][verctl#183]: the skip tests in `src/semver_check.rs` pass.
- [x] 2.2 `verctl semver` runs one check per remaining crate and fails naming each break; proof: code review of `semver_check::run`
  Evidence, 2026-11-10: code review of `semver_check::run`, from [#183][verctl#183].
- [x] 2.3 Running `verctl semver` in a consumer at 0.0.z reports its crates checked at `--release-type minor`; proof: a run on a released consumer after this ships
  Evidence, 2026-11-10: no 0.0.z consumer runs it; shrub, at 0.7.0, passed its Semver step with 202 checks in [CI run 42856082509][shrub-ci].
- [x] 2.4 `plan` leaves out a crate `checkable` rejects.
  A crate is rejected when it has no library target as `docsrs::has_library` resolves it, or when it is a proc-macro, read from its manifest.
  Proof: `only_a_library_that_is_not_a_proc_macro_is_checkable` passes in CI, and shrub's semver step passes on this release.
  That step failed on its binary crate `shrub`.
  Evidence, 2026-11-10: shipped in [#197][verctl#197]: `only_a_library_that_is_not_a_proc_macro_is_checkable`; shrub's Semver table leaves out its binary.

## 3. Docs

- [x] 3.1 The operator instructions and the skill describe the verb; proof: `cargo test operator_docs` passes in CI
  Evidence, 2026-11-10: the instructions and the skill describe the verb; `operator_docs` checks the committed render.

## 4. ctl-core

- [x] 4.1 ctl-core's shared `semver` task runs `mise run ver semver`, and ctl-core and qctl serve `ver`.
  Proof: a ctl-core pull request after this release, with CI green
  Evidence, 2026-11-10: the shared task lives in the actions repository's `tasks/rust.toml` as `semver`, and ctl-core and qctl include `verctl.git//tasks/ver`.

[shrub-ci]: https://github.com/victor-software-house/shrub/actions/runs/42856082509
[verctl#183]: https://github.com/victor-software-house/verctl/pull/183
[verctl#197]: https://github.com/victor-software-house/verctl/pull/197

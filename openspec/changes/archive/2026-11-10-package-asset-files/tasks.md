# Tasks

## 1. Code

- [x] 1.1 Add `assets.dir` and `assets.files` and their load rules to `src/config.rs`, with parse tests; proof: the tests pass in the PR's CI
  Evidence, 2026-11-10: shipped in [#239][verctl#239], whose verify passed: `asset_files_are_packed_from_their_dir` and `asset_files_reject_what_they_cannot_pack`.
- [x] 1.2 Pack the listed files in `src/assets.rs`, with integration tests for the four-file tarball and a missing file.
  Proof: the tests pass in the PR's CI, and `a_package_target_plans_and_packs_as_before` still passes.
  Evidence, 2026-11-10: shipped in [#239][verctl#239]: `a_package_target_packs_the_listed_files` and `a_package_file_missing_after_the_build_is_named` pass, and `a_package_target_plans_and_packs_as_before` still passes.

## 2. Docs

- [x] 2.1 Update the instructions, the operator skill and their templates, `README.md`, and `examples/ver.yaml`; proof: `operator_docs` passes in CI
  Evidence, 2026-11-10: [#239][verctl#239] changed the operator docs and `examples/ver.yaml`; `operator_docs` passed in its verify.
- [x] 2.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it
  Evidence, 2026-11-10: the [#239][verctl#239] fragment passed `verctl check` and is in `CHANGELOG.md`.

[verctl#239]: https://github.com/victor-software-house/verctl/pull/239

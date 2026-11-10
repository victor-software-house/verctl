# Tasks

## 1. Code

- [x] 1.1 Add `files` to an `assets` target and its load rules in `src/config.rs`, with a test per rule.  
  Proof: the tests pass in the PR's CI.
  Evidence, 2026-11-10: shipped in [#313][verctl#313], whose verify passed: `a_target_lists_its_own_files` and `asset_target_files_reject_what_they_cannot_pack`.
- [x] 1.2 Resolve one pack per target in `src/assets.rs`, with a plan test where one of two targets declares its own list.  
  Proof: the test passes in the PR's CI, and `a_package_target_packs_the_listed_files` still passes.
  Evidence, 2026-11-10: shipped in [#313][verctl#313]: the plan test `a_target_packs_its_own_files` and the integration tests `a_target_packs_its_own_files` and `a_target_file_missing_after_the_build_names_its_list` pass.

## 2. Docs

- [x] 2.1 Update the instructions, the operator skill and their templates, `README.md`, and `examples/ver.yaml`; proof: `operator_docs` passes in CI
  Evidence, 2026-11-10: [#313][verctl#313] changed the operator docs, both templates and `examples/ver.yaml`; `operator_docs` passed in its verify.
- [x] 2.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it
  Evidence, 2026-11-10: `.changeset/asset-target-files.md` passed `verctl check` in [#313][verctl#313].

[verctl#313]: https://github.com/victor-software-house/verctl/pull/313

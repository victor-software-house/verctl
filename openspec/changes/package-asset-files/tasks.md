# Tasks

## 1. Code

- [ ] 1.1 Add `assets.dir` and `assets.files` and their load rules to `src/config.rs`, with parse tests; proof: the tests pass in the PR's CI
- [ ] 1.2 Pack the listed files in `src/assets.rs`, with integration tests for the four-file tarball and a missing file; proof: the tests pass in the PR's CI, and `a_package_target_plans_and_packs_as_before` still passes

## 2. Docs

- [ ] 2.1 Update the instructions, the operator skill and their templates, `README.md`, and `examples/ver.yaml`; proof: `operator_docs` passes in CI
- [ ] 2.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it

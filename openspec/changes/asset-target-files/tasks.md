# Tasks

## 1. Code

- [ ] 1.1 Add `files` to an `assets` target and its load rules in `src/config.rs`, with a test per rule.  
  Proof: the tests pass in the PR's CI.
- [ ] 1.2 Resolve one pack per target in `src/assets.rs`, with a plan test where one of two targets declares its own list.  
  Proof: the test passes in the PR's CI, and `a_package_target_packs_the_listed_files` still passes.

## 2. Docs

- [ ] 2.1 Update the instructions, the operator skill and their templates, `README.md`, and `examples/ver.yaml`; proof: `operator_docs` passes in CI
- [ ] 2.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it

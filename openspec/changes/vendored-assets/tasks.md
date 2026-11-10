# Tasks

## 1. Code

- [ ] 1.1 Add the `vendored` schema and its load rules to `src/config.rs`, with parse tests; proof: the tests pass in the PR's CI
- [ ] 1.2 Plan, build, and upload a vendored set in `src/assets.rs`.
  Create the Release and read the digest in `src/github.rs`.
  Add wiremock tests for creation, the creation race, the digest, and existing assets.
  Proof: the tests pass in the PR's CI
- [ ] 1.3 Add `--vendored` to `assets` and its report; proof: an integration test runs `verctl assets --vendored` and the PR's CI passes
- [ ] 1.4 Add a test that pins today's `assets` plan and archive for a stock config; proof: the test passes in the PR's CI

## 2. Docs

- [ ] 2.1 Update the instructions, the operator skill and their templates, and `README.md`; proof: `operator_docs` passes in CI
- [ ] 2.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it

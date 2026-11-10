# Tasks

## 1. Code

- [x] 1.1 Add the `vendored` schema and its load rules to `src/config.rs`, with parse tests; proof: the tests pass in the PR's CI
  Evidence, 2026-11-10: shipped in [#232][verctl#232], whose verify passed, with the parse tests in `src/config.rs`.
- [x] 1.2 Plan, build, and upload a vendored set in `src/assets.rs`.
  Create the Release and read the digest in `src/github.rs`.
  Add wiremock tests for creation, the creation race, the digest, and existing assets.
  Proof: the tests pass in the PR's CI
  Evidence, 2026-11-10: shipped in [#232][verctl#232]: `a_missing_vendored_release_is_created_and_not_made_latest`, `a_vendored_release_another_job_created_first_is_used` and `the_checksum_is_the_digest_github_holds` pass.
- [x] 1.3 Add `--vendored` to `assets` and its report; proof: an integration test runs `verctl assets --vendored` and the PR's CI passes
  Evidence, 2026-11-10: the `--vendored` integration tests in `tests/assets.rs` pass; since [#279][verctl#279] the verbs are `release asset --vendored` and `status --vendored`.
- [x] 1.4 Add a test that pins today's `assets` plan and archive for a stock config; proof: the test passes in the PR's CI
  Evidence, 2026-11-10: `a_package_target_plans_and_packs_as_before` pins the stock plan ([#232][verctl#232]).

## 2. Docs

- [x] 2.1 Update the instructions, the operator skill and their templates, and `README.md`; proof: `operator_docs` passes in CI
  Evidence, 2026-11-10: [#232][verctl#232] changed the instructions, the skill, their templates and `README.md`; `operator_docs` passed.
- [x] 2.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it
  Evidence, 2026-11-10: the [#232][verctl#232] fragment is in `CHANGELOG.md` under verctl 0.7.2.

[verctl#232]: https://github.com/victor-software-house/verctl/pull/232
[verctl#279]: https://github.com/victor-software-house/verctl/pull/279

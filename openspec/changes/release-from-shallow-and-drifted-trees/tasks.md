# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Shallow clones

- [ ] 1.1 `ensure_full_history` fetches the default branch history of a shallow clone; proof: `a_shallow_clone_gets_the_history_that_set_the_version` passes in CI
- [ ] 1.2 A clone that is not shallow is not fetched; proof: `a_full_clone_is_not_fetched_again` passes in CI
- [ ] 1.3 `publish` and `publish --plan` call it before any history walk; proof: code review of `run_pending` and `publish_plan_report`

## 2. Release permission

- [ ] 2.1 A 403 on Release creation names `workflows: write`; proof: `a_forbidden_release_names_the_workflows_permission` passes in CI

## 3. Docs

- [ ] 3.1 The operator instructions and README give the adoption order and the 403 remedy; proof: `cargo test operator_docs` passes in CI

# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Fetch before comparing

- [x] 1.1 `ensure_comparable` fetches a depth-1 merge checkout's base, HEAD's parents included, and the merge-base resolves to HEAD's first parent; proof: `a_pull_request_merge_checkout_compares_against_its_merge_base` passes in CI
- [x] 1.2 A full clone with the ref is not fetched; proof: `a_full_clone_with_the_branch_is_not_fetched_for_a_comparison` passes in CI
- [x] 1.3 A full clone without the ref fetches it; proof: `a_full_clone_without_the_branch_fetches_it_for_a_comparison` passes in CI
- [x] 1.4 `versions::report` fetches `comparison_branches()` before comparing; proof: code review of `report`

## 2. Docs

- [x] 2.1 The operator instructions say the check fetches on Actions; proof: `cargo test operator_docs` passes in CI

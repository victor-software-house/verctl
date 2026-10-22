# Tasks

The pull request's CI runs compile, format, lint, and test tasks with `mise run verify`.

## 1. Plan

- [x] 1.1 `pending` leaves out a package at `0.0.0` before any lookup; proof: `a_package_at_0_0_0_plans_nothing` passes with the GitHub API address closed
  - Evidence: passed in [verify run 39726352572][run-2].
- [x] 1.2 `publish` asks no changelog section of a package at `0.0.0`; proof: `publish_asks_no_changelog_of_a_package_at_0_0_0` passes
  - Evidence: passed in [verify run 39726352572][run-2].
- [x] 1.3 A package at `0.0.0` gets no tag and no dry-run row; proof: `dry_run_leaves_out_a_package_at_0_0_0` passes
  - Evidence: passed in [verify run 39726352572][run-2].
- [ ] 1.4 A dry run of a repository whose only package is at `0.0.0` has nothing to release; proof: `dry_run_of_a_package_at_0_0_0_has_nothing_to_release` passes
- [x] 1.5 `bump::apply` and `pending` share the unreleased test; proof: `cargo test --test bump` passes unchanged
  - Evidence: the full suite, `tests/bump.rs` included, passed in [verify run 39726352572][run-2].

## 2. Document and ship

- [x] 2.1 The instructions template, the operator skill template, and the README state the rule; proof: the `operator_docs` tests pass
  - Evidence: the three `operator_docs` tests passed in [verify run 39726352572][run-2].
- [ ] 2.2 A `patch` changeset ships it; proof: `verctl status` lists it
- [ ] 2.3 Close VER-077 when the pull request merges; proof: `mise run q check` passes

[run-2]: https://github.com/victor-software-house/verctl/actions/runs/39726352572

# Tasks

The pull request's CI runs compile, format, lint, and test tasks with `mise run verify`.

## 1. Plan

- [ ] 1.1 `pending` leaves out a package at `0.0.0` before any lookup; proof: `a_package_at_0_0_0_plans_nothing` passes with the GitHub API address closed
- [ ] 1.2 `publish` asks no changelog section of a package at `0.0.0`; proof: `publish_asks_no_changelog_of_a_package_at_0_0_0` passes
- [ ] 1.3 `bump::apply` and `pending` share the unreleased test; proof: `cargo test --test bump` passes unchanged

## 2. Document and ship

- [ ] 2.1 The instructions template, the operator skill template, and the README state the rule; proof: the `operator_docs` tests pass
- [ ] 2.2 A `patch` changeset ships it; proof: `verctl status` lists it
- [ ] 2.3 Close VER-077 when the pull request merges; proof: `mise run q check` passes

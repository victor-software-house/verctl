# Tasks

Compile, format, lint, and test tasks run on a build runner with
`mise run verify`. The operator reviewed this part with
[reusable-release-workflow](../../reusable-release-workflow/tasks.md), task 0.1,
before it was split out.

## 1. Idempotent publish

- [x] 1.2 The version commit from first-parent history, and `publish --plan` with `$GITHUB_OUTPUT`. Proof: the version-commit scenarios pass against a local repository
  - 2026-10-17 (UTC): [#164][pr-164] merged after [CI run 39068119681][run-164] passed. Local-repository tests: `head_moved_past_the_release_finds_the_commit_that_set_it`, `a_version_set_again_after_a_revert_names_the_newest_change`, `a_tag_on_a_later_commit_at_the_same_version_is_the_release`, `a_tag_on_a_commit_at_another_version_is_not_the_release`, and `the_plan_writes_commits_pending_and_assets`.
- [x] 1.3 `nothing to release` and the `outcome` field. Proof: the nothing-to-release scenarios pass with mocked GitHub responses
  - 2026-10-17 (UTC): the same run passed `a_push_after_a_finished_release_is_nothing_to_release` and `nothing_to_release_says_so_in_both_shapes` against mocked GitHub responses. On `main`, the dispatched [Self-release run 39076359412][run-rerun] printed `outcome  nothing to release`.

[pr-164]: https://github.com/victor-software-house/verctl/pull/164
[run-164]: https://github.com/victor-software-house/verctl/actions/runs/39068119681
[run-rerun]: https://github.com/victor-software-house/verctl/actions/runs/39076359412

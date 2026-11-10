# Tasks

Compile, format, lint, and test tasks run on a build runner with
`mise run verify`. The operator reviewed this part with
[reusable-release-workflow](../reusable-release-workflow/tasks.md), task 0.1,
before it was split out.

## 1. Idempotent publish

- [x] 1.1 `query` on publisher recipes, stock `cargo` and `bun` queries, and the pre- and post-publish checks. Proof: the idempotent-publish registry scenarios pass as tests with a fake publisher and query, including a mocked crates.io index that lacks the version of a workspace member
  - 2026-10-17 (UTC): [#162][pr-162] merged after [CI run 39066949365][run-162] passed on its head. It covers `a_listed_version_skips_the_publisher`, `a_publisher_that_exits_zero_without_publishing_fails`, `a_workspace_member_missing_from_the_index_is_not_published`, the stock and overridden cargo and bun checks, and `a_check_that_keeps_failing_is_reported_after_the_last_round`.

[pr-162]: https://github.com/victor-software-house/verctl/pull/162
[run-162]: https://github.com/victor-software-house/verctl/actions/runs/39066949365

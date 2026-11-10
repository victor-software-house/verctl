# Tasks

Compile, format, lint, and test tasks run on a build runner with
`mise run verify`. The operator reviewed this part with
[reusable-release-workflow](../../reusable-release-workflow/tasks.md), task 0.1,
before it was split out.

## 2. Version PR branch and auto-merge

- [x] 2.1 `verctl/release/<base>` replaces `version-packages` in `release.rs`, `versions.rs`, and the preview. Proof: the version-pr-branch scenarios pass, and `git grep version-packages -- src actions` prints only the migration note in the rendered `src/instructions.md`
  - 2026-10-17 (UTC): on [#161][pr-161] head f136cd6, the test suite passed in [verify run 39036033921][run-161].
    That run covers `version_branch_cli_is_exempt`, `old_version_packages_branch_is_not_exempt`, and the preview's `verctl/release/` head.
  - 2026-10-17 (UTC): `git grep version-packages -- src actions` prints only the migration note in the rendered `src/instructions.md`.
    The note sets a rule: a workflow accepts both heads until the last old Version PR merges.
    Not covered: the `trunk` scenario has no test of its own.
    `version_branch` derives the name from whatever base `github::base_branch` returns.
- [x] 2.2 `prepare --pr --auto-merge` in Rust, and `auto-merge.sh` deleted. Proof: the version-pr-auto-merge scenarios pass against mocked GitHub responses
  - 2026-10-17 (UTC): the same run passed four wiremock tests and one binary test. The wiremock tests are `a_required_check_rule_gates_auto_merge`, `rules_without_a_required_check_do_not_gate_auto_merge`, `auto_merge_squashes_under_the_pull_request_title`, and `a_partial_graphql_response_fails_auto_merge`. The binary test, `auto_merge_refuses_the_job_token_before_writing`, proves the job-token refusal leaves the tree untouched. `auto-merge.sh` is deleted. The run's `semver` step failed only on the API changes that wait for the operator's lint ruling.

[pr-161]: https://github.com/victor-software-house/verctl/pull/161
[run-161]: https://github.com/victor-software-house/verctl/actions/runs/39036033921

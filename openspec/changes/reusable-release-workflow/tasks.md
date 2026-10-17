# Tasks

Compile, format, lint, and test tasks run on a build runner with
`mise run verify`. Implementation starts after the operator reviews this
change.

## 0. Decide

- [ ] 0.1 The operator reviews this change; proof: the spec pull request is approved, and any change of decision is folded into design.md

## 1. Idempotent publish

- [ ] 1.1 `query` on publisher recipes, stock `cargo` and `bun` queries, and the pre- and post-publish checks; proof: the idempotent-publish registry scenarios pass as tests with a fake publisher and query, including a mocked crates.io index that lacks the version of a workspace member
- [ ] 1.2 The version commit from first-parent history, and `publish --plan` with `$GITHUB_OUTPUT`; proof: the version-commit scenarios pass against a local repository
- [ ] 1.3 `nothing to release` and the `outcome` field; proof: the nothing-to-release scenarios pass with mocked GitHub responses
- [ ] 1.4 `verctl docs` and the `docs` job; proof: the docs-build scenarios pass against a mocked docs.rs that answers 404, then `doc_status` true or false

## 2. Version PR branch and auto-merge

- [x] 2.1 `verctl/release/<base>` replaces `version-packages` in `release.rs`, `versions.rs`, and the preview; proof: the version-pr-branch scenarios pass, and `git grep version-packages -- src actions` prints nothing
  - 2026-10-17 (UTC): on [#161][pr-161] head f136cd6, the test suite passed in [verify run 39036033921][run-161]. That run covers `version_branch_cli_is_exempt`, `old_version_packages_branch_is_not_exempt`, and the preview's `verctl/release/` head. `git grep version-packages -- src actions` prints only the migration note in the rendered `src/instructions.md`, which tells a workflow to accept both heads until the last old Version PR merges. Not covered: the `trunk` scenario has no test of its own; `version_branch` derives the name from whatever base `github::base_branch` returns.
- [x] 2.2 `prepare --pr --auto-merge` in Rust, and `auto-merge.sh` deleted; proof: the version-pr-auto-merge scenarios pass against mocked GitHub responses
  - 2026-10-17 (UTC): the same run passed four wiremock tests and one binary test. The wiremock tests are `a_required_check_rule_gates_auto_merge`, `rules_without_a_required_check_do_not_gate_auto_merge`, `auto_merge_squashes_under_the_pull_request_title`, and `a_partial_graphql_response_fails_auto_merge`. The binary test, `auto_merge_refuses_the_job_token_before_writing`, proves the job-token refusal leaves the tree untouched. `auto-merge.sh` is deleted. The run's `semver` step failed only on the API changes that wait for the operator's lint ruling.

## 3. Workflow

- [ ] 3.1 `.github/workflows/release.yml` with the inputs, secrets, jobs, and concurrency of design section 1; proof: zizmor `--pedantic` and the conftest policy pass on it
- [ ] 3.2 Every step runs `verctl <verb>` with `env:` only, on a verctl that adopts ctl-core's environment-bound flags; proof: `git grep -n '\-c "\$' .github actions` prints nothing
- [ ] 3.3 This repository calls its own workflow, with `mise run ver` building verctl, and deletes `version-pr.yml` and `publish.yml`; proof: the next Version PR merge on `main` releases through the new workflow and a rerun reports `nothing to release`

## 4. Docs and consumers

- [ ] 4.1 Instructions, the operator skill, README, AGENTS.md, and `examples/workflows/` describe the caller and the migration; proof: `cargo test operator_docs` passes, and only the migration guide names `version-packages`
- [ ] 4.2 The template renders the caller; proof: a template-rendered repository's caller matches design section 1 and its first release passes

[pr-161]: https://github.com/victor-software-house/verctl/pull/161
[run-161]: https://github.com/victor-software-house/verctl/actions/runs/39036033921

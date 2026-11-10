# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Proofs

- [x] 1.1 The ancestry proof refuses when it cannot run, naming the missing fact. Proof: the pull request for queue row [VER-036][ver-036] adds one test per former skip in `src/git.rs`. The tests pass in CI
  Evidence, 2026-11-10: shipped in [#274][verctl#274]: `no_repository_refuses`, `no_head_commit_refuses` and `a_missing_origin_refuses`.
- [x] 1.2 The workflow-drift proof compares the `.github/workflows/` tree ids; proof: tests for a matching tree and a changed workflow pass in CI
  Evidence, 2026-11-10: shipped in [#277][verctl#277]: `workflows_at_the_default_tip_match` and `a_workflow_changed_since_the_release_commit_differs`. [#315][verctl#315] compares the workflows of the run commit, `GITHUB_SHA`.
- [x] 1.3 The one-version-commit proof counts first-parent version changes since the newest release tag. Proof: tests for one commit, a skipped version, and an untagged package pass in CI
  Evidence, 2026-11-10: redefined as the skipped-version walk ([#277][verctl#277], [#278][verctl#278]); `a_skipped_version_of_the_same_package_refuses` and the walk tests pass.
- [x] 1.4 All proofs run before any publisher, tag, or Release call; proof: a test with a refused proof asserts no publisher ran
  Evidence, 2026-11-10: `a_changed_workflow_refuses_before_any_publish` and `a_commit_off_the_default_branch_refuses_before_any_publish` stop before the GitHub token lookup, which every publisher follows.
- [x] 1.5 `publish --plan` lists a skipped version at the commit that set it, oldest first, with its assets on its own tag. Proof: the git walk and plan grouping tests pass in CI
  Evidence, 2026-11-10: `a_skipped_version_is_planned_first_with_its_own_tag` passes ([#277][verctl#277]).

## 2. Flags

- [x] 2.1 `--allow-workflow-change` and `--allow-off-default-branch` read their `VERCTL_` variables, add `allowed` rows, and fill `allowed` in JSON; proof: cli and report tests pass in CI
  Evidence, 2026-11-10: `release_flags_read_their_variables` ([#279][verctl#279]) and `an_allowed_proof_is_on_the_record_in_both_shapes` ([#277][verctl#277]) pass.
- [x] 2.2 `release.yml` declares both boolean inputs and sets them only in step `env:`; proof: `mise run lint:workflow-policy` passes in CI
  Evidence, 2026-11-10: both inputs default `false` and reach verctl only through step `env:`; `lint:workflow-policy` passed in [#277][verctl#277].

## 3. Bundle

- [x] 3.1 `src/instructions.md`, `skills/verctl/SKILL.md`, and `AGENTS.md` list the three proofs and the two flags; proof: `operator_docs` passes in CI
  Evidence, 2026-11-10: the instructions, the skill and `AGENTS.md` list the three proofs; `operator_docs` passes.
- [x] 3.2 A `minor` fragment names each new refusal, both flags, their variables, and the two inputs; proof: the fragment in the pull request
  Evidence, 2026-11-10: `.changeset/release-commit-proofs.md` names each refusal, both flags, their variables and both inputs.

[ver-036]: ../../../../tasks.yaml

[verctl#274]: https://github.com/victor-software-house/verctl/pull/274
[verctl#277]: https://github.com/victor-software-house/verctl/pull/277
[verctl#278]: https://github.com/victor-software-house/verctl/pull/278
[verctl#279]: https://github.com/victor-software-house/verctl/pull/279
[verctl#315]: https://github.com/victor-software-house/verctl/pull/315

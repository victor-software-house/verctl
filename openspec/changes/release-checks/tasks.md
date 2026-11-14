# Tasks

## 1. Named checks

- [x] 1.1 `verctl check [CHECK]...` with `fragments`, `served`, `versions`, `registry`, `trust` and `--all`; bare `check` runs `fragments` and `served`. Proof: CI tests of `Cli` parsing and of each name's report.
  - This change's first pull request ships `fragments`, `served`, `versions` and `--all`; `registry` and `trust` arrive in the next pull request of the stack, each as one `Check` variant and one dispatch arm.
- [x] 1.2 Remove `--versions` and its `CheckArgs` field, with no hidden flag. Proof: a CI test that `check --versions` exits 2, and `rg -- '--versions' src tests` finds nothing.
- [x] 1.3 Move every reader of `check --versions` in this repository to `check versions`: `src/cli.rs` and its test, `src/main.rs`, `tests/versions.rs`, the doc comments in `src/content.rs` and `src/pull_request.rs`, `src/instructions.md`, `.ctl/operator/instructions.md.jinja`, `.ctl/templates/SKILL.md.jinja`, `skills/verctl/SKILL.md`, `AGENTS.md`, `README.md`, and the comment in `mise-tasks/release.toml`. Proof: `rg 'check --versions'` outside `CHANGELOG.md` and archived changes finds nothing.
  - Outside this repository: the actions repository's `check:versions` task in `tasks/rust.toml` runs `mise run ver check --versions`, and every repository on the templates repository's `verify` runs it. The open changes `derived-release-state`, `release-state-machine` and `version-pr-identity` also name the flag.
- [x] 1.4 Remove the hidden verbs `prepare`, `pin`, `publish` and `assets`, and the hidden `release --plan`: the `Removed` enum, `REMOVED` and `moved_plan` in `src/cli.rs`. Proof: `rg 'Removed|moved_plan' src` finds nothing, and the cli tests assert that each old verb and `--plan` exits 2 as unknown.

## 2. Registry and trust

- [x] 2.1 The `registry` check: existence, owners and trusted-publishing evidence. Proof: integration tests against recorded crates.io index and API responses under `tests/`, run in CI.
- [x] 2.2 The `trust` check with an owner's token. Proof: a CI test against a recorded `github_configs` response, and one with no token.
- [x] 2.3 The `owners` key at the top level and per package. Proof: CI tests of `Config::parse` for the default, the override, and an empty list.
- [x] 2.4 The `release_workflow` key, `release.yml` when omitted, and `self-release.yml` in this repository's `.ctl/ver.yaml`. Proof: CI tests of `Config::parse` for the default, a set value, and a value that is not a file name.

## 3. Release proof

- [x] 3.1 `release` runs the checks `release_checks` names before its first write, `trust` only with `VERCTL_OWNER_TOKEN`; `release --dry-run` prints the plan and runs the same checks. Proof: a CI test where a missing owner stops the run with no draft and no upload, and one where `trust` is skipped without the token.
- [x] 3.2 The `release_checks` key, `[registry, trust]` when omitted, and `check trust` reading `VERCTL_OWNER_TOKEN`. Proof: CI tests of `Config::parse` for the default, `[]`, and a refused name, and the existing trust tests on the new variable.
- [ ] 3.3 Streaming progress and diagnostics are the queue row [VER-138][queue], outside this change. Proof: `mise run q show VER-138` lists it.

## 4. Freeze and tiers

- [x] 4.1 The freeze inside the `registry` check, with the wait for a running release. Proof: a CI test with a recorded runs response.
  - `tests/freeze.rs`: `a_pull_request_freezes_on_a_partial_release`, `a_running_release_is_waited_for`, `the_wait_runs_out`, `the_version_pr_does_not_freeze_on_its_own_versions` and `a_push_only_reports_the_pending_release`.
- [x] 4.2 The `verctl: unfreeze` label passes the freeze. Proof: a CI test with a recorded pull request payload.
  - `tests/freeze.rs`: `the_unfreeze_label_passes_only_the_freeze` and `a_merge_group_reads_the_label_of_its_pull_request`.
- [x] 4.3 `status` names the enforcement tier. Proof: a snapshot test per tier from recorded rules responses.
  - `tests/status.rs`: `status_names_the_merge_queue_tier`, `status_names_the_rulesets_without_a_queue_tier`, `status_names_the_neither_tier` and `status_shows_an_unknown_tier_without_a_token`, over `tests/testdata/rules/`.

## 5. Documents

- [x] 5.1 Update AGENTS.md, the instructions, the operator skill and the README for the named checks, the release proof, the freeze and the tiers. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
- [x] 5.2 A `patch!` fragment for verctl lists the removed flag, verbs and `--plan` under `### Breaking`. Proof: `verctl version --dry-run` shows the entry under that heading.
  - `.changeset/release-checks.md` lists the freeze and the tier. The removed flag, verbs and `--plan` shipped under `### Breaking` in 0.8.6-rc.2 ([#379](https://github.com/victor-software-house/verctl/pull/379)), so this fragment leaves them out.

[queue]: ../../../tasks.yaml

# Tasks

## 1. First publish

- [x] 1.1 `verctl release bootstrap [PACKAGE]...`, with the `Retry-After` wait. Proof: `tests/first_publish.rs` runs the binary against recorded crates.io and GitHub responses and a stand-in `cargo`, and the shrub first publish of `shrub-process` is still to run.
- [x] 1.2 `verctl release trust [PACKAGE]...` and `release trust --dry-run`: list first, add only a missing entry, never edit another. Proof: the `trust_*` tests of `tests/first_publish.rs` against recorded `github_configs` responses for a missing, a matching and a foreign entry, and the `crates_io_add_github_config` tests in `src/registry.rs`.
- [x] 1.3 `check registry` fails a never-published standalone package at a version other than `0.0.0`. Proof: `registry_fails_a_new_standalone_crate_off_zero` and `registry_exempts_a_workspace_member` in `tests/registry.rs`.

## 2. Workflow

- [x] 2.1 The `bootstrap` workflow input, and a release run that plans a bootstrapped standalone package at `0.0.0` with no tag. Proof: `mise run lint:workflow-policy` and `mise run lint:workflows` pass, and `bootstrap_run_plans_a_new_standalone_package` and `a_listed_zero_without_a_tag_is_planned` in `tests/first_publish.rs` run the plan and the release.

## 3. Documents

- [x] 3.1 Update AGENTS.md, the instructions, the operator skill and the README for `release bootstrap`, `release trust` and the `bootstrap` input. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI. The instructions, the skill and the README cover both steps and the `bootstrap` input, and AGENTS.md holds no operator invocation.
- [x] 3.2 A `patch!` fragment for verctl lists the stricter `check` under `### Breaking`. Proof: `.changeset/first-publish.md` is a `verctl: patch!` fragment, and `verctl version --dry-run` shows the entry under that heading.

# Tasks

## 1. Named checks

- [x] 1.1 `verctl check [CHECK]...` with `fragments`, `served`, `versions`, `registry`, `trust` and `--all`; bare `check` runs `fragments` and `served`. Proof: CI tests of `Cli` parsing and of each name's report.
  - This change's first pull request ships `fragments`, `served`, `versions` and `--all`; `registry` and `trust` arrive in the next pull request of the stack, each as one `Check` variant and one dispatch arm.
- [x] 1.2 Remove `--versions` and its `CheckArgs` field, with no hidden flag. Proof: a CI test that `check --versions` exits 2, and `rg -- '--versions' src tests` finds nothing.
- [x] 1.3 Move every reader of `check --versions` in this repository to `check versions`: `src/cli.rs` and its test, `src/main.rs`, `tests/versions.rs`, the doc comments in `src/content.rs` and `src/pull_request.rs`, `src/instructions.md`, `.ctl/operator/instructions.md.jinja`, `.ctl/templates/SKILL.md.jinja`, `skills/verctl/SKILL.md`, `AGENTS.md`, `README.md`, and the comment in `mise-tasks/release.toml`. Proof: `rg 'check --versions'` outside `CHANGELOG.md` and archived changes finds nothing.
  - Outside this repository: the actions repository's `check:versions` task in `tasks/rust.toml` runs `mise run ver check --versions`, and every repository on the templates repository's `verify` runs it. The open changes `derived-release-state`, `release-state-machine` and `version-pr-identity` also name the flag.
- [x] 1.4 Remove the hidden verbs `prepare`, `pin`, `publish` and `assets`, and the hidden `release --plan`: the `Removed` enum, `REMOVED` and `moved_plan` in `src/cli.rs`. Proof: `rg 'Removed|moved_plan' src` finds nothing, and the cli tests assert that each old verb and `--plan` exits 2 as unknown.

## 2. Registry and trust

- [ ] 2.1 The `registry` check: existence, owners and trusted-publishing evidence. Proof: integration tests against recorded crates.io index and API responses under `tests/`, run in CI.
- [ ] 2.2 The `trust` check with an owner's token. Proof: a CI test against a recorded `github_configs` response, and one with no token.
- [ ] 2.3 The `owners` key at the top level and per package. Proof: CI tests of `Config::parse` for the default, the override, and an empty list.
- [ ] 2.4 The `release_workflow` key, `release.yml` when omitted, and `self-release.yml` in this repository's `.ctl/ver.yaml`. Proof: CI tests of `Config::parse` for the default, a set value, and a value that is not a file name.

## 3. Release proof

- [ ] 3.1 `release --dry-run` runs the checks, `cargo publish --workspace --dry-run`, the `version` replay and the asset builds, and uploads nothing. Proof: CI tests, and a dry run on this repository's own Version PR.
- [ ] 3.2 `release` runs the same proof before its first upload. Proof: a CI test where a missing crate stops the run with no upload.
- [ ] 3.3 Streaming progress and diagnostics are the queue row [VER-138][queue], outside this change. Proof: `mise run q show VER-138` lists it.

## 4. Freeze and tiers

- [ ] 4.1 The freeze inside the `registry` check, with the wait for a running release. Proof: a CI test with a recorded runs response.
- [ ] 4.2 The `verctl: unfreeze` label passes the freeze. Proof: a CI test with a recorded pull request payload.
- [ ] 4.3 `status` names the enforcement tier. Proof: a snapshot test per tier from recorded rules responses.

## 5. Documents

- [ ] 5.1 Update AGENTS.md, the instructions, the operator skill and the README for the named checks, the release proof, the freeze and the tiers. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
- [ ] 5.2 A `patch!` fragment for verctl lists the removed flag, verbs and `--plan` under `### Breaking`. Proof: `verctl version --dry-run` shows the entry under that heading.

[queue]: ../../../tasks.yaml

# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Proofs

- [ ] 1.1 The ancestry proof refuses when it cannot run, naming the missing fact; proof: the VER-036 pull request adds one test per former skip in `src/git.rs`, and they pass in CI
- [ ] 1.2 The workflow-drift proof compares the `.github/workflows/` tree ids; proof: tests for a matching tree and a changed workflow pass in CI
- [ ] 1.3 The one-version-commit proof counts first-parent version changes since the newest release tag; proof: tests for one commit, a skipped version, and an untagged package pass in CI
- [ ] 1.4 All proofs run before any publisher, tag, or Release call; proof: a test with a refused proof asserts no publisher ran

## 2. Flags

- [ ] 2.1 `--allow-workflow-change` and `--allow-off-default-branch` read their `VERCTL_` variables, print `allowed:` lines, and fill `allowed` in JSON; proof: cli and report tests pass in CI
- [ ] 2.2 `release.yml` declares both boolean inputs and sets them only in step `env:`; proof: `mise run lint:workflow-policy` passes in CI

## 3. Bundle

- [ ] 3.1 `src/instructions.md`, `skills/verctl/SKILL.md`, and `AGENTS.md` list the three proofs and the two flags; proof: `operator_docs` passes in CI
- [ ] 3.2 A `minor` fragment names each new refusal, both flags, their variables, and the two inputs; proof: the fragment in the pull request

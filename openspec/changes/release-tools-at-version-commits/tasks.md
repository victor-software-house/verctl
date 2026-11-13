# Tasks

The pull request's CI verifies every compile, format, lint, and test task.

## 1. The plan

- [ ] 1.1 `status --github-output` writes `release-tools`, a map from version commit to its publishers' tools, with `all` for a custom publisher.
  Verify: a `status` snapshot test with a cargo package, a bun package, and a custom publisher passes in CI.

## 2. The workflow

- [ ] 2.1 The publish job installs the plan's list, or `release-tools` when set, adds `mr-boxington` when the version commit's rust entry sets `mr_boxington`, and runs the release with `MISE_EXEC_AUTO_INSTALL=false`.
  Verify: `mise run lint:workflows` and `mise run lint:workflow-policy` pass in CI.
- [ ] 2.2 The docs job skips `mise install` at the version commit.
  Verify: `rg -n 'mise install' .github/workflows/release.yml` shows no install step in the docs job beyond the plan's verctl.
- [ ] 2.3 The `release-tools` input and the header comment state the list, the override, and `all`.
  Verify: reading the header and the input description.

## 3. Proof

- [ ] 3.1 shrub moves its release caller to the commit that ships this, and its 0.7.0 release publishes the 7 missing crates.
  Verify: the shrub Release run link and `cargo search` output for each crate, recorded here.
- [ ] 3.2 A `.changeset/` patch fragment.
  Verify: `verctl check` in CI passes on the pull request.

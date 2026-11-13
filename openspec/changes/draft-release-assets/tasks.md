# Tasks

The pull request's CI verifies every compile, format, lint, and test task.

## 1. Lookups and the draft lifecycle

- [ ] 1.1 Release lookups read drafts through the list fallback and count only `uploaded` assets.
  Verify: the `github` draft lookup tests pass in CI against a mocked API.
- [ ] 1.2 `release` creates the tag, then a draft, for a Release with declared assets, and publishes it once every asset is uploaded.
  Verify: the `github` draft creation and finalize tests pass in CI.
- [ ] 1.3 A Release with no declared asset, or with `--skip-assets`, is published at once.
  Verify: the `github` publish-at-once test passes in CI.

## 2. Completeness

- [ ] 2.1 A draft is incomplete, a mutable published Release plans only its missing targets, and an immutable one counts complete with a warning.
  Verify: the `pending` draft, mutable, and immutable tests pass in CI.
- [ ] 2.2 A run at a finished version commit reports `nothing to release`.
  Verify: the `drafts` finished-commit test passes in CI.
- [ ] 2.3 A draft at an older version is planned again after a newer version commit lands.
  Verify: the `drafts` older-draft test passes in CI.
- [ ] 2.4 The plan hands each commit its expected assets, and a leg declares exactly them.
  Verify: the `pending` declared-output test and the `publish` declared tests pass in CI.
- [ ] 2.5 `release --releases-only` starts no publisher.
  Verify: the `drafts` releases-only test passes in CI.

## 3. Workflow and docs

- [ ] 3.1 `release.yml` finalizes after `assets`, and `plan` requests `contents: write`.
  Verify: `mise run lint:workflows` and `mise run lint:workflow-policy` pass in CI.
- [ ] 3.2 The operator instructions, the skill, and the README describe drafts and the plan permission.
  Verify: `cargo test operator_docs` passes in CI.
- [ ] 3.3 The next verctl release goes from draft to published with every tarball.
  Verify: the release run's link, recorded here.
- [ ] 3.4 A `.changeset/` fragment of the breaking patch kind.
  Verify: `verctl check` in CI passes on the pull request.

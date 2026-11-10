# Prove the commit a release publishes from

## Why

`verctl publish` proves two things about the release commit: each package's
CHANGELOG has a heading for its version, and HEAD is an ancestor of the
default branch. A consumer kept both and added three checks of its own
([docs/adoption-report.md](../../../docs/adoption-report.md) section 4).
The workflow-drift check cost that consumer a release. A retry of an old
release commit runs the workflow as it was at that commit. If permissions or
steps changed since, the run can publish to the registry and then fail to
tag. The registry cannot be reverted, and the tags are the record.

The ancestry check can also pass without running. `prove_default_history`
returns success when there is no repository, no HEAD, or no default-branch
ref, and says nothing (VER-036).

The operator placed these checks in the 0.8.0 minor on 2026-11-09, because
they refuse releases that run today (operator, 2026-11-09).

Queue row: VER-035.

## What changes

1. **BREAKING:** `verctl publish` refuses a release commit whose
   `.github/workflows/` tree differs from the default branch's.
   `--allow-workflow-change` (`VERCTL_ALLOW_WORKFLOW_CHANGE`) lets it run.
2. `--allow-off-default-branch` (`VERCTL_ALLOW_OFF_DEFAULT_BRANCH`) lets a
   release run from a commit the default branch does not contain. VER-036,
   which ships first, makes the ancestry check refuse when it cannot run.
3. **BREAKING:** `verctl publish` refuses a package with a skipped version:
   an earlier version set on the first-parent line since its last release
   tag, with no tag, that no later commit abandoned by going below it. No
   flag allows it.
4. A run that uses either flag reports which check it went past and why:
   an `allowed` row in the human view, and `allowed` in the JSON report.
5. `verctl publish --plan` lists a version an earlier commit set and no
   release has, at the commit that set it, so the workflow releases it
   before the newer one.
6. The reusable `release.yml` gains two boolean inputs,
   `allow-workflow-change` and `allow-off-default-branch`, default `false`,
   passed to verctl through step `env:` only.

## Impact

1. Code: `src/publish.rs`, `src/git.rs`, `src/pending.rs`, `src/cli.rs`,
   `src/presentation.rs`, `src/main.rs`.
2. Workflow: `.github/workflows/release.yml`.
3. Docs: `src/instructions.md`, `skills/verctl/SKILL.md`, `AGENTS.md`, and
   this change's `release-commit-proofs` spec.

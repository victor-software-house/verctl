# Tasks

## 1. Before this change

- [ ] 1.1 Archive the open changes that already shipped: `draft-release-assets`, `release-build-once`, `release-jobs-run-head-verctl`, `release-asset-targets`, `registry-token-secret` and `version-pr-proposal`. Then archive `breaking-patch`, `command-renames` and `version-pr-body-cap`, each after verifying in code that it shipped. The deltas of this change and of `release-checks` then apply to current specs. Proof: `openspec list` no longer shows them, and `openspec validate --strict release-from-head` passes.

## 2. Release

- [x] 2.1 `status` and `release` take HEAD as the release reference; `refuse_skipped_versions`, the oldest-first plan and the version-commit refusal are gone. Proof: `rg refuse_skipped_versions src` finds nothing, and the CI tests of `pending` cover HEAD only.
- [x] 2.2 `status --github-output` writes HEAD as the only commit. Proof: a CI test with two packages bumped at different commits.

## 3. Workflow

- [ ] 3.1 `release.yml` runs at HEAD only: no matrix of version commits, no checkout of an old commit. Proof: `actionlint` and the workflow policy in CI, and a release of this repository.

## 4. Documents

- [x] 4.1 Update AGENTS.md, the instructions, the operator skill and the README for HEAD as the release reference. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
- [ ] 4.2 A `patch!` fragment for verctl lists the HEAD-only release under `### Breaking`. Proof: `verctl version --dry-run` shows the entry under that heading.

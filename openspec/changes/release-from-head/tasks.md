# Tasks

## 1. Before this change

- [ ] 1.1 Archive the open changes that already shipped: `draft-release-assets`, `release-build-once`, `release-jobs-run-head-verctl`, `release-asset-targets`, `registry-token-secret` and `version-pr-proposal`. Then archive `breaking-patch`, `command-renames` and `version-pr-body-cap`, each after verifying in code that it shipped. The deltas of this change and of `release-checks` then apply to current specs. Proof: `openspec list` no longer shows them, and `openspec validate --strict release-from-head` passes.

## 2. Release

- [x] 2.1 `status` and `release` take HEAD as the release reference; `refuse_skipped_versions`, the oldest-first plan and the version-commit refusal are gone. Proof: `rg refuse_skipped_versions src` finds nothing, and the CI tests of `pending` cover HEAD only.
- [x] 2.2 `status --github-output` writes HEAD as the only commit. Proof: a CI test with two packages bumped at different commits.

## 3. Workflow

- [x] 3.1 `release.yml` runs at HEAD only: no matrix of version commits, no checkout of an old commit. Proof: `actionlint` and the workflow policy in CI, and a release of this repository.
  - [#371][pr-371] passed both in CI, and [run 43511698400][release-run] released `0.8.6-rc.2` from HEAD `19ee926`.

## 4. Documents

- [x] 4.1 Update AGENTS.md, the instructions, the operator skill and the README for HEAD as the release reference. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
- [x] 4.2 A `patch!` fragment for verctl lists the HEAD-only release under `### Breaking`. Proof: `verctl version --dry-run` shows the entry under that heading.
  - [Version PR #384][pr-384] listed it under `### Breaking`.

[pr-371]: https://github.com/victor-software-house/verctl/pull/371
[pr-384]: https://github.com/victor-software-house/verctl/pull/384
[release-run]: https://github.com/victor-software-house/verctl/actions/runs/43511698400

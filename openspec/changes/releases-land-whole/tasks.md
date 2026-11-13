# Tasks

## 1. Before this change

- [ ] 1.1 Archive the open changes that already shipped: `draft-release-assets`, `release-build-once`, `release-jobs-run-head-verctl`, `release-asset-targets`, `registry-token-secret` and `version-pr-proposal`, so this change's deltas apply to current specs. Proof: `openspec list` no longer shows them, and `openspec validate --strict releases-land-whole` passes.

## 2. Checks

- [ ] 2.1 `verctl check --registry`: existence, trusted publisher, owners, and the freeze. Proof: integration tests against a recorded crates.io index and API responses under `tests/`, run in CI.
- [ ] 2.2 `verctl check --release`: `check --registry`, `cargo publish --workspace --dry-run`, the `version` replay, and the asset builds into a draft. Proof: CI tests, and a dry run on this repository's own Version PR.
- [ ] 2.3 The `verctl: unfreeze` label passes the freeze. Proof: a CI test with a recorded pull request payload.

## 3. Release

- [ ] 3.1 `status` and `release` take HEAD as the release reference; `refuse_skipped_versions` and the oldest-first plan are gone. Proof: `rg refuse_skipped_versions src` finds nothing, and the CI tests of `pending` cover HEAD only.
- [ ] 3.2 Order: draft and assets, registries, then the draft published with its tag. Proof: a CI test that records the calls in order.
- [ ] 3.3 A partial release is a `status` state with its cause. Proof: a snapshot test of the human and JSON views.
- [ ] 3.4 `release` runs `check --release` before its first upload. Proof: a CI test where a missing crate stops the run with no upload.

## 4. First publish

- [ ] 4.1 `verctl release --first` and `verctl trust`, with the `Retry-After` wait. Proof: CI tests against recorded responses, and the shrub first publish of `shrub-process`.
- [ ] 4.2 `check` fails a never-published standalone package at a version other than `0.0.0`. Proof: a CI test.
- [ ] 4.3 The `first-publish` workflow input. Proof: the workflow's policy check and a dry run.

## 5. Workflow

- [ ] 5.1 `release.yml` runs at HEAD only: no matrix of version commits, no checkout of an old commit. Proof: `actionlint` and the workflow policy in CI, and a release of this repository.
- [ ] 5.2 `status` names the enforcement tier. Proof: a snapshot test per tier from recorded rules responses.

## 6. Documents

- [ ] 6.1 Update AGENTS.md, the instructions, the operator skill and the README for the new checks, the order, the freeze and the first publish. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
